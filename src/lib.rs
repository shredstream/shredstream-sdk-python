
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pyo3::create_exception;
use pyo3::exceptions::{
    PyBrokenPipeError, PyConnectionResetError, PyException, PyOSError, PyRuntimeError,
};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use shredstream::{
    classify_variant as core_classify_variant, pin_current_thread_to_cpu as core_pin_cpu,
    AccumulatorConfig, ListenerOptions, ShredListener, VariantKind,
};

create_exception!(_native, PanicError, PyException);
create_exception!(_native, ListenerClosedError, PyException);


fn map_io_err(e: io::Error) -> PyErr {
    match e.kind() {
        io::ErrorKind::BrokenPipe => PyBrokenPipeError::new_err(e.to_string()),
        io::ErrorKind::ConnectionReset => PyConnectionResetError::new_err(e.to_string()),
        _ => PyOSError::new_err(e.to_string()),
    }
}

fn io_kind_str(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::BrokenPipe => "BrokenPipe",
        io::ErrorKind::ConnectionReset => "ConnectionReset",
        io::ErrorKind::NotConnected => "NotConnected",
        io::ErrorKind::ConnectionAborted => "ConnectionAborted",
        io::ErrorKind::AddrInUse => "AddrInUse",
        io::ErrorKind::AddrNotAvailable => "AddrNotAvailable",
        io::ErrorKind::TimedOut => "TimedOut",
        io::ErrorKind::WouldBlock => "WouldBlock",
        io::ErrorKind::Interrupted => "Interrupted",
        io::ErrorKind::UnexpectedEof => "UnexpectedEof",
        _ => "Other",
    }
}

fn catch<F, T>(f: F) -> PyResult<T>
where
    F: FnOnce() -> PyResult<T>,
{
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in shredstream native code".to_string()
            };
            Err(PanicError::new_err(msg))
        }
    }
}

fn serialize_txs(
    py: Python<'_>,
    txs: Vec<solana_transaction::versioned::VersionedTransaction>,
) -> PyResult<Vec<Py<PyBytes>>> {
    let mut out = Vec::with_capacity(txs.len());
    for tx in txs {
        let bytes = bincode::serialize(&tx)
            .map_err(|e| PyRuntimeError::new_err(format!("bincode serialize: {e}")))?;
        out.push(PyBytes::new(py, &bytes).into());
    }
    Ok(out)
}


#[pyclass(name = "VariantKind", module = "shredstream._native")]
#[derive(Clone)]
struct PyVariantKind {
    inner: VariantKind,
}

#[pymethods]
impl PyVariantKind {
    #[getter]
    fn is_data(&self) -> bool {
        self.inner.is_data()
    }

    #[getter]
    fn is_code(&self) -> bool {
        self.inner.is_code()
    }

    #[getter]
    fn proof_size(&self) -> u8 {
        self.inner.proof_size()
    }

    #[getter]
    fn resigned(&self) -> bool {
        self.inner.resigned()
    }

    #[getter]
    fn merkle_suffix(&self) -> usize {
        self.inner.merkle_suffix()
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            VariantKind::DataLegacy => "DataLegacy",
            VariantKind::CodeLegacy => "CodeLegacy",
            VariantKind::DataMerkle { .. } => "DataMerkle",
            VariantKind::CodeMerkle { .. } => "CodeMerkle",
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "VariantKind(kind={}, proof_size={}, resigned={})",
            self.kind(),
            self.proof_size(),
            self.resigned()
        )
    }
}


#[pyclass(name = "AccumulatorConfig", module = "shredstream._native")]
#[derive(Clone)]
struct PyAccumulatorConfig {
    #[pyo3(get, set)]
    max_fec_sets_per_slot: usize,
    #[pyo3(get, set)]
    stuck_batch_timeout_ms: u64,
}

#[pymethods]
impl PyAccumulatorConfig {
    #[new]
    #[pyo3(signature = (max_fec_sets_per_slot = None, stuck_batch_timeout_ms = None))]
    fn new(max_fec_sets_per_slot: Option<usize>, stuck_batch_timeout_ms: Option<u64>) -> Self {
        let d = AccumulatorConfig::default();
        Self {
            max_fec_sets_per_slot: max_fec_sets_per_slot.unwrap_or(d.max_fec_sets_per_slot),
            stuck_batch_timeout_ms: stuck_batch_timeout_ms
                .unwrap_or_else(|| d.stuck_batch_timeout.as_millis() as u64),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "AccumulatorConfig(max_fec_sets_per_slot={}, stuck_batch_timeout_ms={})",
            self.max_fec_sets_per_slot, self.stuck_batch_timeout_ms
        )
    }
}

impl PyAccumulatorConfig {
    fn to_core(&self) -> AccumulatorConfig {
        AccumulatorConfig {
            max_fec_sets_per_slot: self.max_fec_sets_per_slot,
            stuck_batch_timeout: Duration::from_millis(self.stuck_batch_timeout_ms),
        }
    }
}


#[pyclass(name = "ListenerOptions", module = "shredstream._native")]
#[derive(Clone)]
struct PyListenerOptions {
    #[pyo3(get, set)]
    recv_buf: usize,
    #[pyo3(get, set)]
    max_age: u64,
    #[pyo3(get, set)]
    busy_poll_us: Option<u32>,
    #[pyo3(get, set)]
    pool_size: usize,
    #[pyo3(get, set)]
    enable_fec: bool,
    #[pyo3(get, set)]
    disable_salvage_delivery: bool,
    #[pyo3(get, set)]
    accumulator: PyAccumulatorConfig,
}

#[pymethods]
impl PyListenerOptions {
    #[new]
    #[pyo3(signature = (
        recv_buf = None,
        max_age = None,
        busy_poll_us = None,
        pool_size = None,
        enable_fec = None,
        disable_salvage_delivery = None,
        accumulator = None,
        busy_poll_disabled = false,
    ))]
    fn new(
        recv_buf: Option<usize>,
        max_age: Option<u64>,
        busy_poll_us: Option<u32>,
        pool_size: Option<usize>,
        enable_fec: Option<bool>,
        disable_salvage_delivery: Option<bool>,
        accumulator: Option<PyAccumulatorConfig>,
        busy_poll_disabled: bool,
    ) -> Self {
        let d = ListenerOptions::default();
        let busy = if busy_poll_disabled {
            None
        } else {
            busy_poll_us.or(d.busy_poll_us)
        };
        Self {
            recv_buf: recv_buf.unwrap_or(d.recv_buf),
            max_age: max_age.unwrap_or(d.max_age),
            busy_poll_us: busy,
            pool_size: pool_size.unwrap_or(d.pool_size),
            enable_fec: enable_fec.unwrap_or(d.enable_fec),
            disable_salvage_delivery: disable_salvage_delivery
                .unwrap_or(d.disable_salvage_delivery),
            accumulator: accumulator.unwrap_or_else(|| PyAccumulatorConfig::new(None, None)),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ListenerOptions(recv_buf={}, max_age={}, busy_poll_us={:?}, pool_size={}, \
             enable_fec={}, disable_salvage_delivery={}, accumulator={})",
            self.recv_buf,
            self.max_age,
            self.busy_poll_us,
            self.pool_size,
            self.enable_fec,
            self.disable_salvage_delivery,
            self.accumulator.__repr__()
        )
    }
}

impl PyListenerOptions {
    fn to_core(&self) -> ListenerOptions {
        ListenerOptions {
            recv_buf: self.recv_buf,
            max_age: self.max_age,
            busy_poll_us: self.busy_poll_us,
            pool_size: self.pool_size,
            enable_fec: self.enable_fec,
            disable_salvage_delivery: self.disable_salvage_delivery,
            accumulator: self.accumulator.to_core(),
        }
    }
}


#[pyclass(name = "RawShred", module = "shredstream._native")]
struct PyRawShred {
    #[pyo3(get)]
    slot: u64,
    #[pyo3(get)]
    index: u32,
    #[pyo3(get)]
    payload_len: usize,
}

#[pymethods]
impl PyRawShred {
    fn __repr__(&self) -> String {
        format!(
            "RawShred(slot={}, index={}, payload_len={})",
            self.slot, self.index, self.payload_len
        )
    }
}


type ListenerSlot = Arc<Mutex<Option<ShredListener>>>;

#[pyclass(name = "ShredListener", module = "shredstream._native")]
struct PyShredListener {
    inner: ListenerSlot,
}

impl PyShredListener {
    fn slot(listener: ShredListener) -> ListenerSlot {
        Arc::new(Mutex::new(Some(listener)))
    }

    fn with<F, R>(&self, f: F) -> PyResult<R>
    where
        F: FnOnce(&ShredListener) -> R,
    {
        let guard = self
            .inner
            .lock()
            .map_err(|_| PyRuntimeError::new_err("listener mutex poisoned"))?;
        match guard.as_ref() {
            Some(l) => Ok(f(l)),
            None => Err(ListenerClosedError::new_err("ShredListener has been closed")),
        }
    }

}

#[pymethods]
impl PyShredListener {
    #[new]
    #[pyo3(signature = (port, options = None))]
    fn new(port: u16, options: Option<PyListenerOptions>) -> PyResult<Self> {
        catch(|| {
            let listener = match options {
                Some(o) => ShredListener::bind_with_options(port, o.to_core()),
                None => ShredListener::bind(port),
            }
            .map_err(map_io_err)?;
            Ok(Self {
                inner: Self::slot(listener),
            })
        })
    }

    #[staticmethod]
    fn bind(port: u16) -> PyResult<Self> {
        catch(|| {
            let listener = ShredListener::bind(port).map_err(map_io_err)?;
            Ok(Self {
                inner: Self::slot(listener),
            })
        })
    }

    #[staticmethod]
    fn bind_with_options(port: u16, options: PyListenerOptions) -> PyResult<Self> {
        catch(|| {
            let listener =
                ShredListener::bind_with_options(port, options.to_core()).map_err(map_io_err)?;
            Ok(Self {
                inner: Self::slot(listener),
            })
        })
    }

    #[staticmethod]
    fn offline() -> PyResult<Self> {
        catch(|| {
            let listener = ShredListener::bind(0).map_err(map_io_err)?;
            Ok(Self {
                inner: Self::slot(listener),
            })
        })
    }

    #[cfg(unix)]
    #[staticmethod]
    #[pyo3(signature = (fd, options=None))]
    fn from_fd(fd: i32, options: Option<PyListenerOptions>) -> PyResult<Self> {
        use std::os::unix::io::FromRawFd;
        catch(|| {
            let std_socket = unsafe { std::net::UdpSocket::from_raw_fd(fd) };
            let opts = options.map(|o| o.to_core()).unwrap_or_default();
            let listener =
                ShredListener::from_socket(std_socket, opts).map_err(map_io_err)?;
            Ok(Self {
                inner: Self::slot(listener),
            })
        })
    }

    fn close(&self) {
        if let Ok(mut g) = self.inner.lock() {
            let _ = g.take();
        }
    }

    fn local_addr(&self) -> PyResult<String> {
        self.with(|l| l.local_addr().map(|a| a.to_string()))?
            .map_err(map_io_err)
    }

    #[getter]
    fn slot_count(&self) -> PyResult<usize> {
        self.with(|l| l.slot_count())
    }

    fn handle_packet<'py>(
        &self,
        py: Python<'py>,
        data: &[u8],
    ) -> PyResult<Option<(u64, Vec<Py<PyBytes>>)>> {
        let owned = data.to_vec();
        let slot = self.inner.clone();
        let result = py.detach(|| -> PyResult<Option<(u64, Vec<solana_transaction::versioned::VersionedTransaction>)>> {
            catch(|| {
                let mut guard = slot
                    .lock()
                    .map_err(|_| PyRuntimeError::new_err("listener mutex poisoned"))?;
                match guard.as_mut() {
                    Some(l) => Ok(l.handle_packet(&owned)),
                    None => Err(ListenerClosedError::new_err("ShredListener has been closed")),
                }
            })
        })?;
        match result {
            Some((s, txs)) => Ok(Some((s, serialize_txs(py, txs)?))),
            None => Ok(None),
        }
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<(u64, Vec<Py<PyBytes>>)> {
        let slot = self.inner.clone();
        let outcome = py.detach(|| -> Result<(u64, Vec<solana_transaction::versioned::VersionedTransaction>), Option<io::ErrorKind>> {
            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut guard = match slot.lock() {
                    Ok(g) => g,
                    Err(_) => return Err(Some(io::ErrorKind::Other)),
                };
                let l = match guard.as_mut() {
                    Some(l) => l,
                    None => return Err(None),
                };
                match l.transactions().next() {
                    Some(batch) => Ok(batch),
                    None => Err(l.last_io_error_kind()),
                }
            }));
            match res {
                Ok(v) => v,
                Err(_) => Err(Some(io::ErrorKind::Other)),
            }
        });
        match outcome {
            Ok((slot, txs)) => Ok((slot, serialize_txs(py, txs)?)),
            Err(None) => Err(pyo3::exceptions::PyStopIteration::new_err(
                "listener closed",
            )),
            Err(Some(kind)) => {
                let msg = format!("listener stopped: {}", io_kind_str(kind));
                match kind {
                    io::ErrorKind::BrokenPipe => Err(PyBrokenPipeError::new_err(msg)),
                    io::ErrorKind::ConnectionReset => Err(PyConnectionResetError::new_err(msg)),
                    _ => Err(PyOSError::new_err(msg)),
                }
            }
        }
    }

    fn shreds(&self) -> PyShredIter {
        PyShredIter {
            slot: self.inner.clone(),
        }
    }


    #[getter]
    fn pool_exhausted_count(&self) -> PyResult<u64> {
        self.with(|l| l.pool_exhausted_count())
    }

    #[getter]
    fn last_io_error_kind(&self) -> PyResult<Option<&'static str>> {
        self.with(|l| l.last_io_error_kind().map(io_kind_str))
    }

    #[getter]
    fn busy_poll_active(&self) -> PyResult<bool> {
        self.with(|l| l.busy_poll_active())
    }

    #[getter]
    fn data_shred_count_total(&self) -> PyResult<u64> {
        self.with(|l| l.data_shred_count_total())
    }

    #[getter]
    fn code_shred_count_total(&self) -> PyResult<u64> {
        self.with(|l| l.code_shred_count_total())
    }

    #[getter]
    fn bytes_received(&self) -> PyResult<u64> {
        self.with(|l| l.bytes_received())
    }

    #[getter]
    fn unparseable_packets(&self) -> PyResult<u64> {
        self.with(|l| l.unparseable_packets())
    }

    #[getter]
    fn unparseable_too_short(&self) -> PyResult<u64> {
        self.with(|l| l.unparseable_too_short())
    }

    #[getter]
    fn unparseable_variant(&self) -> PyResult<u64> {
        self.with(|l| l.unparseable_variant())
    }

    #[getter]
    fn unparseable_payload(&self) -> PyResult<u64> {
        self.with(|l| l.unparseable_payload())
    }

    #[getter]
    fn unparseable_slot_range(&self) -> PyResult<u64> {
        self.with(|l| l.unparseable_slot_range())
    }

    #[getter]
    fn dropped_known_slots(&self) -> PyResult<u64> {
        self.with(|l| l.dropped_known_slots())
    }

    #[getter]
    fn harvested_batches_total(&self) -> PyResult<u64> {
        self.with(|l| l.harvested_batches_total())
    }

    #[getter]
    fn decode_errors_total(&self) -> PyResult<u64> {
        self.with(|l| l.decode_errors_total())
    }

    #[getter]
    fn fec_recoveries_total(&self) -> PyResult<u64> {
        self.with(|l| l.fec_recoveries_total())
    }

    #[getter]
    fn fec_recovery_failures_total(&self) -> PyResult<u64> {
        self.with(|l| l.fec_recovery_failures_total())
    }

    #[getter]
    fn batches_skipped_total(&self) -> PyResult<u64> {
        self.with(|l| l.batches_skipped_total())
    }

    #[getter]
    fn batches_decoded_streaming_total(&self) -> PyResult<u64> {
        self.with(|l| l.batches_decoded_streaming_total())
    }

    #[getter]
    fn batches_decoded_fallback_total(&self) -> PyResult<u64> {
        self.with(|l| l.batches_decoded_fallback_total())
    }

    #[getter]
    fn slots_completed_total(&self) -> PyResult<u64> {
        self.with(|l| l.slots_completed_total())
    }

    #[getter]
    fn slots_evicted_by_age(&self) -> PyResult<u64> {
        self.with(|l| l.slots_evicted_by_age())
    }

    #[getter]
    fn salvaged_tail_tx_total(&self) -> PyResult<u64> {
        self.with(|l| l.salvaged_tail_tx_total())
    }

    #[getter]
    fn fec_sets_discarded_unused_total(&self) -> PyResult<u64> {
        self.with(|l| l.fec_sets_discarded_unused_total())
    }

    #[getter]
    fn fec_sets_evicted_early_total(&self) -> PyResult<u64> {
        self.with(|l| l.fec_sets_evicted_early_total())
    }

    #[getter]
    fn batches_force_finalized_corrupted_total(&self) -> PyResult<u64> {
        self.with(|l| l.batches_force_finalized_corrupted_total())
    }

    #[getter]
    fn batches_force_finalized_timeout_total(&self) -> PyResult<u64> {
        self.with(|l| l.batches_force_finalized_timeout_total())
    }

}


#[pyclass(name = "ShredIter", module = "shredstream._native")]
struct PyShredIter {
    slot: ListenerSlot,
}

#[pymethods]
impl PyShredIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<PyRawShred> {
        let slot = self.slot.clone();
        let outcome = py.detach(|| -> Result<shredstream::listener::RawShred, Option<io::ErrorKind>> {
            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut guard = match slot.lock() {
                    Ok(g) => g,
                    Err(_) => return Err(Some(io::ErrorKind::Other)),
                };
                let l = match guard.as_mut() {
                    Some(l) => l,
                    None => return Err(None),
                };
                match l.shreds().next() {
                    Some(s) => Ok(s),
                    None => Err(l.last_io_error_kind()),
                }
            }));
            match res {
                Ok(v) => v,
                Err(_) => Err(Some(io::ErrorKind::Other)),
            }
        });
        match outcome {
            Ok(s) => Ok(PyRawShred {
                slot: s.slot,
                index: s.index,
                payload_len: s.payload_len,
            }),
            Err(None) => Err(pyo3::exceptions::PyStopIteration::new_err(
                "listener closed",
            )),
            Err(Some(kind)) => {
                let msg = format!("listener stopped: {}", io_kind_str(kind));
                match kind {
                    io::ErrorKind::BrokenPipe => Err(PyBrokenPipeError::new_err(msg)),
                    io::ErrorKind::ConnectionReset => Err(PyConnectionResetError::new_err(msg)),
                    _ => Err(PyOSError::new_err(msg)),
                }
            }
        }
    }
}


#[pyfunction]
fn pin_current_thread_to_cpu(cpu_id: usize) -> PyResult<()> {
    catch(|| core_pin_cpu(cpu_id).map_err(map_io_err))
}

#[pyfunction]
fn classify_variant(byte: u8) -> Option<PyVariantKind> {
    core_classify_variant(byte).map(|inner| PyVariantKind { inner })
}


#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyShredListener>()?;
    m.add_class::<PyShredIter>()?;
    m.add_class::<PyListenerOptions>()?;
    m.add_class::<PyAccumulatorConfig>()?;
    m.add_class::<PyVariantKind>()?;
    m.add_class::<PyRawShred>()?;
    m.add_function(wrap_pyfunction!(pin_current_thread_to_cpu, m)?)?;
    m.add_function(wrap_pyfunction!(classify_variant, m)?)?;
    m.add("PanicError", py.get_type::<PanicError>())?;
    m.add(
        "ListenerClosedError",
        py.get_type::<ListenerClosedError>(),
    )?;
    m.add("__version__", "2.0.0")?;
    Ok(())
}
