// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/blocking_written_child_input.sv
module leaf(input logic [7:0] value);
    logic [7:0] echoed;
    always_comb echoed = value;
endmodule

module tb;
    logic [7:0] source = 8'h11;
    logic [7:0] control = 8'hc3;
    leaf child(.value(source));

    // Focal vector: integral_bit_logic, direct_projection, port_actual,
    // whole_object, module_package, input, module, child_port, none, none,
    // none, procedural_blocking, none.
    initial begin
        #1;
        if (source !== 8'h11 || child.echoed !== 8'h11 || control !== 8'hc3)
            $fatal(1, "blocking child-input baseline mismatch");

        source = 8'h5a;
        #1;
        if (source !== 8'h5a || child.echoed !== 8'h5a || control !== 8'hc3)
            $fatal(1, "blocking child-input update mismatch");

        $display("blocking_port=%02h/%02h", source, child.echoed);
        $finish(0);
    end
endmodule
