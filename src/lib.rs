use std::io;

use io::Read;

use io::Write;

use csv::Reader;
use csv::StringRecord;

use wasmtime::StoreLimits;

use wasmtime::Memory;
use wasmtime::MemoryAccessError;

use wasmtime::AsContextMut;

pub struct CsvReader<R>(Reader<R>);

impl<R> CsvReader<R>
where
    R: Read,
{
    pub fn into_records(self) -> impl Iterator<Item = Result<StringRecord, io::Error>> {
        self.0
            .into_records()
            .map(|rslt| rslt.map_err(io::Error::other))
    }
}

pub fn reader2record<R>(rdr: R) -> impl Iterator<Item = Result<StringRecord, io::Error>>
where
    R: Read,
{
    CsvReader(Reader::from_reader(rdr)).into_records()
}

pub fn stdin2records() -> impl Iterator<Item = Result<StringRecord, io::Error>> {
    reader2record(io::stdin().lock())
}

pub struct WasmPage(pub [u8; 65536]);

pub trait CsvToCborFlat {
    type Error: core::error::Error;

    fn to_wasm_page(
        &self,
        record: &StringRecord,
        page: &mut WasmPage,
    ) -> Result<usize, Self::Error>;

    fn record2wasm<S, E>(
        &self,
        mem: &Memory,
        store: S,
        offset: usize,
        buf: &mut WasmPage,
        record: &StringRecord,
        emap: E,
    ) -> Result<(), Self::Error>
    where
        S: AsContextMut,
        E: FnOnce(MemoryAccessError) -> Self::Error,
    {
        let sz: usize = self.to_wasm_page(record, buf)?;
        let data: &[u8] = &buf.0[..sz];
        mem.write(store, offset, data).map_err(emap)?;
        Ok(())
    }
}

#[derive(Default)]
pub struct CsvToCborFlatBasic {}

impl CsvToCborFlat for CsvToCborFlatBasic {
    type Error = io::Error;

    fn to_wasm_page(
        &self,
        record: &StringRecord,
        page: &mut WasmPage,
    ) -> Result<usize, Self::Error> {
        let mut msl: &mut [u8] = &mut page.0;

        let mut tot: usize = 0;

        for values in record.iter() {
            let bval: &[u8] = values.as_bytes();

            msl.write_all(&[0x79])?; // utf8 string, up to 65535 bytes
            tot += 1;

            let bvsz: usize = bval.len();
            let shszs: u16 = bvsz.try_into().map_err(io::Error::other)?;
            let ash: [u8; 2] = shszs.to_be_bytes();

            msl.write_all(&ash)?;
            tot += ash.len();

            msl.write_all(bval)?;
            tot += bval.len();
        }
        Ok(tot)
    }
}

pub struct State<I, C> {
    pub records: I,
    pub limits: StoreLimits,
    pub cbor_conv: C,
    pub buf: WasmPage,
}
