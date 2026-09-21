// llg-test-fixture: SYN-019 Verilog-2001 rejection of a type parameter
// IEEE 1800-2009 §6.20.3; parameter type declarations are SystemVerilog-only.
module tb #(parameter type T = reg);
    T value;
    initial begin
        value = 1'b0;
        $finish;
    end
endmodule
