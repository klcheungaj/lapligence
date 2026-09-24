// llg-test-fixture: IEEE 1800-2009 §23.3.3.2.
// A module ref port aliases a whole variable actual: updates propagate from
// parent to child, and a child write through the alias updates the parent.
// Focal vector: integral_bit_logic, direct_projection, port_actual, whole_object,
// module_package, ref, module, child_port, none, none, none, none, none.
module ref_child(ref logic [7:0] shared, output logic [7:0] mirror);
    always_comb mirror = shared;

    initial begin
        #4;
        shared = 8'hc3;
    end
endmodule

module tb;
    logic [7:0] value;
    logic [7:0] mirror;

    ref_child u_ref(.shared(value), .mirror(mirror));

    initial begin
        value = 8'h5a;
        #1;
        if (mirror !== 8'h5a)
            $fatal(1, "ref port did not expose the parent value");

        value = 8'ha5;
        #1;
        if (mirror !== 8'ha5)
            $fatal(1, "ref port did not track a parent update");

        #3;
        if (value !== 8'hc3 || mirror !== 8'hc3)
            $fatal(1, "child write through ref port did not update the parent alias");

        $display("refport=%02h", value);
        $finish(0);
    end
endmodule
