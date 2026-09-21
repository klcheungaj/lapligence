// llg-test-fixture: SYN-019 Verilog-2001 rejection of packed struct
// IEEE 1800-2009 §7.2.1; packed structures are not Verilog-2001 declarations.
module tb;
    struct packed {
        reg [3:0] value;
    } packet;
    initial begin
        packet.value = 4'h0;
        $finish;
    end
endmodule
