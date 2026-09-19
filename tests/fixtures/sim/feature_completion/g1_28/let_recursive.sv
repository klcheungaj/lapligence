// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_28/let_recursive.sv
// A let that expands itself has no finite expansion; elaboration must reject it
// with its declaration location instead of recursing or partially lowering.
module tb;
    logic [7:0] a;
    let recur = recur + 8'd1;

    initial begin
        a = recur;
        $display("a=%0d", a);
        $finish(0);
    end
endmodule
