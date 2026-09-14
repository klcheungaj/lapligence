// llg-test-fixture: tests/fixtures/sim/review_batch4/multiclock_flow.sv
module tb;
    bit ca = 0, cb = 0, data = 1;
    int hits;
    cover property (@(posedge ca) data ##1 @(posedge cb) data ##2 data) hits++;
    initial begin
        #1 ca = 1;
        #1 cb = 1;
        #1 cb = 0;
        #1 cb = 1;
        #1 cb = 0;
        #1 cb = 1;
        #1;
        if (hits != 1) $fatal(1, "secondary-clock concatenation did not inherit its clock");
        $display("multiclock flow ok");
        $finish(0);
    end
endmodule
