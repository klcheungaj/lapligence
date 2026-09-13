// llg-test-fixture: tests/fixtures/sim/process_semantics/function_timing.sv
// IEEE 1800-2009 Sections 9.2.2.2 and 13.4: a called task with blocking
// timing is not legal in an always_comb process.
module tb;
    logic a;
    logic y;

    task automatic delayed(input logic value);
        #1;
        y = value;
    endtask

    always_comb delayed(a);

    initial begin
        a = 1'b0;
        #1 $finish(0);
    end
endmodule
