use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::io::Read;
use std::path::Path;

pub fn render_csv<R: Read>(input: R) -> Result<String, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(input);
    let headers = rdr.headers()?.clone().iter().map(|f| f.to_string()).collect::<Vec<String>>();

    let records = rdr.records();
    
    let mut csv_table = CsvTable::new(headers);

    for record in records {
        csv_table.append(record?.iter().map(|f| f.to_string()).collect::<Vec<String>>())?;
    }

    Ok(csv_table.to_string())
}

pub fn render_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    render_csv(file)
}

struct CsvTable {
    headers: Vec<String>,
    records: Vec<Vec<String>>,
    col_lens: Vec<usize>
}

impl CsvTable {
    fn new(headers: Vec<String>) -> CsvTable {
        CsvTable{headers: headers.clone(), 
                 records: Vec::new(), 
                 col_lens: headers.iter().map(|f| f.len()).collect()}
    }

    fn append(&mut self, record: Vec<String>) -> Result<(), Box<dyn Error>> {
        
        if record.len()!=self.col_lens.len() {
            return Err("CSV record size mismatch.")?;
        }

        let cur_col_lens = record.iter().map(|r| r.len()).collect::<Vec<usize>>();
        for (i, l) in self.col_lens.iter_mut().enumerate() {
            *l = (*l).max(cur_col_lens[i]);
        }

        self.records.push(record);
        
        Ok(())
    }
}

impl Display for CsvTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        let format_table_line = |record: &Vec<String>| {
            format!("| {} |", record.iter().enumerate().map(|(i,s)| {
                format!("{:>width$}", s, width=self.col_lens[i])
            }).collect::<Vec<String>>().join(" | "))
        };

        let header = format_table_line(&self.headers);
        let separator = "-".repeat(header.len());

        let body = self.records.iter().map(format_table_line)
            .collect::<Vec<String>>().join("\n");

        write!(f, "{separator}\n{header}\n{separator}\n{body}\n{separator}\n")
    }
}

