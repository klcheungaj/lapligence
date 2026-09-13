// IEEE 1364-2001 6.2.1 and IEEE 1800-2009 6.8, 6.21, and 10.5: a variable
// declaration initializer may race with an active process in Verilog-2001,
// while SystemVerilog evaluates static initialization before ordinary code.
module tb;
    reg source;
    reg initialized = source;

    initial begin
        source = 1'b1;
        if (initialized !== 1'bx && initialized !== 1'b1) begin
            $display("FAIL declaration_init_edition got=%b", initialized);
            $finish;
        end
        $display("PASS declaration_init_edition");
        $finish;
    end
endmodule
