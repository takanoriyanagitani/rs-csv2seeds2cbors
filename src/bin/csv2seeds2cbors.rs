use std::io;
use std::process::ExitCode;

use io::Write;

use wasmtime::Engine;
use wasmtime::Instance;
use wasmtime::Linker;
use wasmtime::TypedFunc;

use wasmtime::Module;
use wasmtime::ModuleExport;

use wasmtime::Caller;

use wasmtime::Extern;
use wasmtime::Memory;
use wasmtime::Store;

use wasmtime::StoreLimits;
use wasmtime::StoreLimitsBuilder;

use csv::StringRecord;

use rs_csv2seeds2cbors::CsvToCborFlat;
use rs_csv2seeds2cbors::CsvToCborFlatBasic;
use rs_csv2seeds2cbors::State;
use rs_csv2seeds2cbors::WasmPage;
use rs_csv2seeds2cbors::stdin2records;

fn on_state<I, C>(state: State<I, C>) -> Result<(), io::Error>
where
    I: Iterator<Item = Result<StringRecord, io::Error>> + 'static,
    C: CsvToCborFlat + 'static,
{
    let we: Engine = Engine::default();

    let wat_path: String = std::env::var("ENV_WAT_PATH").unwrap_or_default();

    let wat_bytes: Vec<u8> = std::fs::read(wat_path)?;

    let mut wl: Linker<_> = Linker::<State<I, C>>::new(&we);

    let mut store: Store<State<I, C>> = Store::new(&we, state);
    store.limiter(|state: &mut State<_, _>| &mut state.limits);

    let wm: Module = Module::new(&we, wat_bytes).map_err(io::Error::other)?;

    let omodexp: Option<ModuleExport> = wm.get_export_index("memory");
    let Some(mexp) = omodexp else {
        return Err(io::Error::other("memory not found"));
    };

    wl.func_wrap("host", "print_i32", |_: Caller<'_, _>, value: i32| {
        println!("i32: {value}");
    })
    .map_err(io::Error::other)?;

    wl.func_wrap("host", "print_i64", |_: Caller<'_, _>, value: i64| {
        println!("i64: {value}");
    })
    .map_err(io::Error::other)?;

    wl.func_wrap(
        "host",
        "a2unsigned",
        move |mut caller: Caller<'_, State<I, C>>, offset: i32, siz: i32| {
            let oext: Option<Extern> = caller.get_module_export(&mexp);
            let omem: Option<Memory> = oext.and_then(|ext| match ext {
                Extern::Memory(m) => Some(m),
                _ => None,
            });

            let Some(mem) = omem else {
                return -1; // No mem found
            };

            let mut buf: [u8; 65536] = [0; 65536];
            let minsz: usize = buf.len().min(siz as usize);
            let rres: Result<(), _> = mem.read(&mut caller, offset as usize, &mut buf[..minsz]);
            let Ok(_) = rres else {
                return -2; // Unable to read the string
            };

            let sl: &[u8] = &buf[..minsz];
            let s: &str = std::str::from_utf8(sl).unwrap_or_default();
            let ri: Result<i64, _> = s.parse();
            let Ok(i) = ri else {
                return -3; // Invalid string
            };

            i
        },
    )
    .map_err(io::Error::other)?;

    wl.func_wrap(
        "host",
        "cbor_docs_wrote",
        move |mut caller: Caller<'_, State<I, C>>, offset: i32, siz: i32| {
            let oext: Option<Extern> = caller.get_module_export(&mexp);
            let omem: Option<Memory> = oext.and_then(|ext| match ext {
                Extern::Memory(m) => Some(m),
                _ => None,
            });

            let Some(mem) = omem else {
                return -1; // No mem found
            };

            let mut buf: [u8; 65536] = [0; 65536];
            let minsz: usize = buf.len().min(siz as usize);
            let rres: Result<(), _> = mem.read(&mut caller, offset as usize, &mut buf[..minsz]);
            let Ok(_) = rres else {
                return -2; // Unable to read the cbor docs
            };

            let mut ol = std::io::stdout().lock();
            let rwrit: Result<_, _> = ol.write_all(&buf[..minsz]);
            match rwrit {
                Ok(_) => 0,
                Err(_) => -3, // Unable to write the cbor docs
            }
        },
    )
    .map_err(io::Error::other)?;

    wl.func_wrap(
        "host",
        "request_seeds",
        move |mut caller: Caller<'_, State<I, C>>, offset: i32| {
            let oext: Option<Extern> = caller.get_module_export(&mexp);
            let omem: Option<Memory> = oext.and_then(|ext| match ext {
                Extern::Memory(m) => Some(m),
                _ => None,
            });

            let Some(mem) = omem else {
                return -1; // No mem found
            };

            let state: &mut State<I, C> = caller.data_mut();
            let iter: &mut I = &mut state.records;
            let ores: Option<Result<_, _>> = iter.next();
            let Some(res) = ores else {
                return -9; // EOF
            };

            let Ok(rec) = res else {
                return -2; // Invalid record
            };

            let page: &mut WasmPage = &mut state.buf;
            let conv: &C = &state.cbor_conv;

            let ricborsz: Result<usize, _> = conv.to_wasm_page(&rec, page);
            let Ok(icborsz) = ricborsz else {
                return -3; // Unable to serialize the csv row
            };

            let icbors: &[u8] = &page.0[..icborsz];
            let mut cpbuf: [u8; 65536] = [0; 65536];
            let minsz: usize = icbors.len().min(cpbuf.len());
            cpbuf[..minsz].copy_from_slice(icbors);
            let rwrt: Result<(), _> = mem.write(&mut caller, offset as usize, &cpbuf);
            let Ok(_) = rwrt else {
                return -4; // Unable to write to wasm memory
            };

            minsz as i32
        },
    )
    .map_err(io::Error::other)?;

    let wi: Instance = wl.instantiate(&mut store, &wm).map_err(io::Error::other)?;
    let io_main: TypedFunc<(), ()> = wi
        .get_typed_func(&mut store, "io_main")
        .map_err(io::Error::other)?;
    io_main.call(&mut store, ()).map_err(io::Error::other)?;

    Ok(())
}

fn sub() -> Result<(), io::Error> {
    let bldr: StoreLimitsBuilder = StoreLimitsBuilder::new().memory_size(1048576).instances(1);
    let lmts: StoreLimits = bldr.build();

    // Iterator<Item=Result<StringRecord, io::Error>>
    let records = stdin2records();

    let state = State {
        records,
        limits: lmts,
        cbor_conv: CsvToCborFlatBasic::default(),
        buf: WasmPage([0; 65536]),
    };

    on_state(state)
}

fn main() -> ExitCode {
    sub().map(|_| ExitCode::SUCCESS).unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })
}
