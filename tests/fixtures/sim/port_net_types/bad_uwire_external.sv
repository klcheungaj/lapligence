// llg-test-fixture: R05 retains the frontend's unresolved-inout boundary.
module plain(inout wire p); endmodule
module tb;
    uwire p;
    plain u0(p);
    initial #1 $finish;
endmodule
