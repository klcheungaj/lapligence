// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_runtime_lane_chain.sv
// Each packed index beneath a fixed-array element selects its own dimension.
module tb;
    logic [3:0][1:0][7:0] deep [0:1];
    integer i;
    integer j;
    integer k;

    initial begin
        deep[1] = 64'haabb_ccdd_1122_3344;
        i = 1;
        j = 2;
        k = 1;
        $display("sel %h", deep[i][j][k]);
        $finish(0);
    end
endmodule
