// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_runtime_lane_bounds.sv
// IEEE 1800-2009 7.4.6 and 11.5.3: an out-of-range or unknown runtime packed
// lane index reads X and writes nothing; an out-of-range element index makes
// the whole composed selection a no-op. Host memory is never touched.
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    logic [7:0] bits [0:1];
    integer i;
    integer j;

    initial begin
        packed_elem[0] = 32'h1122_3344;
        packed_elem[1] = 32'haabb_ccdd;
        bits[1] = 8'h5a;
        i = 1;

        // High out-of-range: X read, no write.
        j = 8;
        $display("high %h %h", packed_elem[i][j], bits[i][j]);
        packed_elem[i][j] = 8'h00;
        bits[i][j] = 1'b0;
        $display("highwr %h %h", packed_elem[1], bits[1]);

        // Low out-of-range: X read, no write.
        j = -1;
        $display("low %h %h", packed_elem[i][j], bits[i][j]);
        packed_elem[i][j] = 8'hff;
        bits[i][j] = 1'b1;
        $display("lowwr %h %h", packed_elem[1], bits[1]);

        // Unknown index: X read, no write.
        j = 32'bxxxxxxxx;
        $display("unknown %h %h", packed_elem[i][j], bits[i][j]);
        packed_elem[i][j] = 8'hff;
        bits[i][j] = 1'b1;
        $display("unknownwr %h %h", packed_elem[1], bits[1]);

        // An out-of-range element with a runtime lane index stays a no-op.
        i = 2;
        j = 2;
        $display("elemhigh %h %h", packed_elem[i][j], bits[i][j]);
        packed_elem[i][j] = 8'hff;
        bits[i][j] = 1'b1;
        $display("elemhighwr %h %h", packed_elem[1], bits[1]);

        // An in-range selection still observes the element lane.
        i = 1;
        j = 2;
        $display("in %h %h", packed_elem[i][j], bits[i][j]);
        $finish(0);
    end
endmodule
