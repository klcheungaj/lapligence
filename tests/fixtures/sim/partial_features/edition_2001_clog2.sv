// llg-test-fixture: SYN-019 Verilog-2001 rejection of $clog2
// IEEE 1800-2009 §20.8; $clog2 is a SystemVerilog system function.
module tb;
    localparam integer WIDTH = $clog2(9);
    reg [WIDTH-1:0] value;
    initial begin
        value = WIDTH;
        $finish;
    end
endmodule
