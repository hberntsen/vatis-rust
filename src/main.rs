use linux_stats;

fn main()
{
    let stat = linux_stats::stat();

    let stat = match stat
    {
        Ok(s) => s,
        Err(error) => {
            panic!("error while getting stats: {:?}", error)
        },
    };

    let mem_info = linux_stats::meminfo();

    let mem_info = match mem_info {
        Ok(m) => m,
        Err(error) => {
            panic!("error while getting memory stats: {:?}", error)
        }
    };

}
