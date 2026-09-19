// llg-test-fixture: gate array with scalar control broadcast across vector
// outputs in declared order. LRM: IEEE 1364-1995 7.1, IEEE 1800-2009 28.3.
module tb;
    reg [3:0] a;
    reg b;
    wire [3:0] y;

    and ga[3:0] (y, a, b);

    initial begin
        a = 4'b1010;
        b = 1'b1;
        #1;
        $display("CHECK: %b", y);
        b = 1'b0;
        #1;
        $display("CHECK: %b", y);
        b = 1'bx;
        #1;
        $display("CHECK: %b", y);
        $finish(0);
    end
endmodule
