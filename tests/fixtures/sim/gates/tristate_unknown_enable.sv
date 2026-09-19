// llg-test-fixture: unknown/high-impedance enable controls on tri-state gates
// must resolve to X, never to an arbitrary Boolean arm. LRM: IEEE 1364-1995
// 7.4 Table 7-5.
module tb;
    reg data, en;
    wire y_b1, y_b0, y_n1, y_n0;

    bufif1 gb1(y_b1, data, en);
    bufif0 gb0(y_b0, data, en);
    notif1 gn1(y_n1, data, en);
    notif0 gn0(y_n0, data, en);

    initial begin
        data = 1'b1;
        en = 1'b1;
        #1;
        $display("CHECK: on %b %b %b %b", y_b1, y_b0, y_n1, y_n0);
        en = 1'b0;
        #1;
        $display("CHECK: off %b %b %b %b", y_b1, y_b0, y_n1, y_n0);
        en = 1'bx;
        #1;
        $display("CHECK: x %b %b %b %b", y_b1, y_b0, y_n1, y_n0);
        en = 1'bz;
        #1;
        $display("CHECK: z %b %b %b %b", y_b1, y_b0, y_n1, y_n0);
        $finish(0);
    end
endmodule
