use std::env;
use std::fmt::Display;
use std::process::exit;
use std::io::Write;
use slug::slugify;
use std::error::Error;

fn print_usage_and_exit() {
    eprintln!("Missing an argument.");
    eprintln!("Usage: transform lowercase|uppercase|no-spaces|slugify|csv");

    exit(1);
}

fn transform_slugify(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(match input.chars().last() {
        Some('\n') => format!("{}\n", slugify(input)),
        _ => input.to_string()
    })
}

fn transform_lowercase(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(input.to_lowercase())
}

fn transform_uppercase(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(input.to_uppercase())
}

fn transform_no_spaces(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(input.replace(" ", ""))
}


fn transform_string(operation: &str) -> Result<(), Box<dyn Error>> {
    let transformation: fn(input: &str) -> Result<String, Box<dyn Error>> = match operation {
        "lowercase" => transform_lowercase,
        "uppercase" => transform_uppercase,
        "no-spaces" => transform_no_spaces,
        "slugify" => transform_slugify,
        _ => {
            return Err("Unknown operation.")?;
        }
    };

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
 
    loop {
        let mut buf = String::new();

        let bytes_read = stdin.read_line(&mut buf)?;
        if bytes_read==0 {
            return Ok(());
        } else {
            let transformed = (transformation)(&buf)?;
            stdout.write_all(transformed.as_bytes())?;
            stdout.flush()?;
        }
    }
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
        
        return Ok(());
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

        return write!(f, "{separator}\n{header}\n{separator}\n{body}\n{separator}\n");
    }
}

fn transform_csv() -> Result<(), Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(std::io::stdin());
    let headers = rdr.headers()?.clone().iter().map(|f| f.to_string()).collect::<Vec<String>>();

    let records = rdr.records();
    
    let mut csv_table = CsvTable::new(headers);

    for record in records {
        csv_table.append(record?.iter().map(|f| f.to_string()).collect::<Vec<String>>())?;
    }

    println!("{}", csv_table);

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        print_usage_and_exit()
    }

    let operation = args[1].as_str();
    let result = match operation {
        "csv" => transform_csv(),
        _ => transform_string(operation)
    };

    match result {
        Err(msg) => {
            eprintln!("{}", msg);
            exit(1);
        }
        Ok(_) => {
            exit(0);
        }
    }

}