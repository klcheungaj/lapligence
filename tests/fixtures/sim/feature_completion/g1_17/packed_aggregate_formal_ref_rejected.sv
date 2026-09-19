// IEEE 1800-2009 13.5.2: a packed `ref` formal aliases caller storage.
typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
} pair_t;

module tb;
    pair_t v;

    function automatic void f(ref pair_t p);
        p.hi = 8'h01;
    endfunction

    initial begin
        v.hi = 8'h00;
        v.lo = 8'h00;
        f(v);
        if (v.hi !== 8'h01 || v.lo !== 8'h00) $fatal(1, "packed ref did not alias caller");
        $display("packed ref passed");
        $finish(0);
    end
endmodule
