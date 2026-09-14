// llg-test-fixture: tests/fixtures/sim/review_batch4/multiclock_time.sv
module tb;
    bit ca = 0, cb = 0, cc = 0, cd = 0, ce = 0, cf = 0;
    int later_zero, later_one, before_zero, before_one, after_zero, after_one;
    cover property (@(posedge ca) 1'b1 ##0 @(posedge cb) 1'b1) later_zero++;
    cover property (@(posedge ca) 1'b1 ##1 @(posedge cb) 1'b1) later_one++;
    cover property (@(posedge cc) 1'b1 ##0 @(posedge cd) 1'b1) before_zero++;
    cover property (@(posedge cc) 1'b1 ##1 @(posedge cd) 1'b1) before_one++;
    cover property (@(posedge ce) 1'b1 ##0 @(posedge cf) 1'b1) after_zero++;
    cover property (@(posedge ce) 1'b1 ##1 @(posedge cf) 1'b1) after_one++;
    initial begin
        #5 ca = 1;
        #2 cb = 1;
        #1;
        if (later_zero != 1 || later_one != 1) $fatal(1, "nearest later destination edge");
        #2 begin cd = 1; cc = 1; ce = 1; cf = 1; end
        #1;
        if (before_zero != 1 || after_zero != 1 || before_one != 0 || after_one != 0)
            $fatal(1, "coincident edges depended on delivery order");
        #1 begin cd = 0; cf = 0; end
        #1 begin cd = 1; cf = 1; end
        #1;
        if (before_one != 1 || after_one != 1) $fatal(1, "strictly later edge not used");
        $display("multiclock physical time ok");
        $finish(0);
    end
endmodule
