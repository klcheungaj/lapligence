// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_05/owner_publication_snapshot.sv
// G1-05 owner_publication_snapshot: a wide value published to an NBA from a
// block-local owner must survive both the source overwrite and the end of the
// owner's lexical scope. IEEE 1800-2009 10.4.2.
module tb;
    logic [129:0] source;
    logic [129:0] captured;

    initial begin
        source = 130'd5;
        captured = '0;
        begin : publish
            logic [129:0] temporary;
            temporary = source + 130'd2;
            captured <= temporary;
        end
        source = 130'd0;
        #1;
        $display("%0d", captured);
        $display("%0d", source);
        $finish(0);
    end
endmodule
