// llg-test-fixture: disjoint selected drivers across fixed-array net elements
// must keep independent driver identities. LRM: IEEE 1800-2009 6.6, 10.3.
module tb;
    wire [7:0] lane [0:2];
    reg [7:0] a, b;

    assign lane[0] = a;
    assign lane[1] = b;
    assign lane[2][7:4] = a[3:0];
    assign lane[2][3:0] = b[3:0];

    initial begin
        a = 8'hA5;
        b = 8'h3C;
        #1;
        $display("CHECK: %h %h %h", lane[0], lane[1], lane[2]);
        a = 8'h00;
        #1;
        $display("CHECK: %h %h %h", lane[0], lane[1], lane[2]);
        b = 8'hF0;
        #1;
        $display("CHECK: %h %h %h", lane[0], lane[1], lane[2]);
        $finish(0);
    end
endmodule
