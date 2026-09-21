// llg-test-fixture: SYN-019 Verilog-2001 rejection of whole-array equality
// IEEE 1800-2009 §7.6; unpacked array equality is a SystemVerilog value use.
module tb;
    reg [7:0] a [0:1];
    reg [7:0] b [0:1];
    initial begin
        if (a == b) $display("unexpected");
        $finish;
    end
endmodule
