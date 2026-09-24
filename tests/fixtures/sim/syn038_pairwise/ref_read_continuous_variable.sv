// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv
// A read-only ref task may observe a continuously driven logic variable.
module tb;
    logic [7:0] source;
    logic [7:0] observed;

    assign source = 8'h5a;

    task automatic capture_ref(ref logic [7:0] value);
        observed = value;
    endtask

    initial begin
        #1;
        capture_ref(source);
        if (source !== 8'h5a || observed !== 8'h5a)
            $fatal(1, "read-only ref source mismatch");
        $display("ref-read=%02h", observed);
        $finish(0);
    end
endmodule
