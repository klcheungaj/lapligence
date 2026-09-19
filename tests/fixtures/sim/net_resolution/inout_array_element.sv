// llg-test-fixture: inout port actuals that are fixed-array elements keep their
// own identity without disturbing sibling elements. LRM: IEEE 1800-2009 23.3.3.
module child(inout wire p);
    assign p = 1'bz;
endmodule

module tb;
    reg drv;
    wire lane [0:1];

    child u0(.p(lane[0]));
    child u1(.p(lane[1]));
    assign lane[0] = drv;

    initial begin
        drv = 1'b1;
        #1;
        $display("CHECK: %b %b", lane[0], lane[1]);
        drv = 1'b0;
        #1;
        $display("CHECK: %b %b", lane[0], lane[1]);
        $finish(0);
    end
endmodule
