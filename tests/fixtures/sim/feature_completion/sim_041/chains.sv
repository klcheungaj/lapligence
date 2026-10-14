// SIM-041 A03: repeated and recursive SV -> C -> SV re-entry, export output
// and inout copy-back (including C passing one address for two outputs),
// string, byte and packed formals and results, an exported zero-time task under an imported
// task, and simulation time that stays still across the chains.
module tb;
    import "DPI-C" context function int ch_loop(input int n);
    import "DPI-C" context function int ch_rec(input int n);
    import "DPI-C" context function void ch_alias();
    import "DPI-C" context task ch_task(input int n, output int o);

    export "DPI-C" function ch_step;
    export "DPI-C" function ch_down;
    export "DPI-C" function ch_pair;
    export "DPI-C" function ch_text;
    export "DPI-C" function ch_wide;
    export "DPI-C" function ch_byte;
    export "DPI-C" task ch_tick;

    int acc = 0;

    function int ch_step(input int i, output int sq, inout int total);
        sq = i * i;
        total += sq;
        acc += 1;
        return i + 1;
    endfunction

    // SV -> C -> SV -> C ... n levels deep.
    function int ch_down(input int n);
        return n <= 0 ? 0 : n + ch_rec(n - 1);
    endfunction

    function void ch_pair(input int a, output int x, output int y);
        x = a;
        y = a + 1;
    endfunction

    function string ch_text(input string s, output string o, inout string io);
        o = {s, "-out"};
        io = {io, "+"};
        return {s, "-ret"};
    endfunction

    function void ch_wide(input logic [99:0] a, output logic [99:0] b, inout bit [64:0] c);
        b = ~a;
        c = c + 1;
    endfunction

    // SV byte is C char (Table H.1); its value must not depend on the
    // signedness of plain char.
    function byte ch_byte(input byte b, output byte o);
        $display("ch_byte b=%0d", b);
        o = b - 8'sd1;
        return b + b;
    endfunction

    task ch_tick(input int k, output int o);
        o = 2 * k;
        acc += k;
    endtask

    int r;
    int out = 0;
    time t0;

    initial begin
        #5;
        t0 = $time;
        r = ch_loop(1000);
        $display("ch_loop=%0d acc=%0d", r, acc);
        r = ch_rec(50);
        $display("ch_rec=%0d", r);
        ch_alias();
        ch_task(7, out);
        $display("ch_task o=%0d acc=%0d", out, acc);
        $display("time %0t -> %0t", t0, $time);
        $finish;
    end
endmodule
