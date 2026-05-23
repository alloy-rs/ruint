macro_rules! const_range_for {
    ($i:ident in $start:tt.. $end:expr => $x:block) => {
        const_range_for!(@range $i, $start, $end, $x)
    };
    ($i:ident in ($start:expr).. $end:expr => $x:block) => {
        const_range_for!(@range $i, $start, $end, $x)
    };
    (@range $i:ident, $start:expr, $end:expr, $x:block) => {{
        let mut iter = $start;
        while iter < $end {
            let $i = iter;
            iter += 1;
            $x
        }
    }};
    ($i:ident in $slice:expr => $x:block) => {{
        let slice = $slice;
        const_range_for!(i in 0..slice.len() => {
            let $i = &slice[i];
            $x
        })
    }};
}
