// llg-test-fixture: SYN-019 Verilog-2001 rejection of typedef
// IEEE 1800-2009 §6.18; `typedef` is not a Verilog-2001 declaration keyword.
module tb;
    typedef reg [3:0] word_t;
    word_t value;
    initial begin
        value = 4'h0;
        $finish;
    end
endmodule
