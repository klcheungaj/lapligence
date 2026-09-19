// llg-test-fixture: a selected alias chain shares one canonical network across
// several partial connections. LRM: IEEE 1800-2009 10.11, 23.3.3.7.
module tb;
    wire [7:0] a;
    wire [3:0] b;
    wire [1:0] c;
    reg [3:0] drv;

    alias a[3:0] = b;
    alias b[1:0] = c;

    assign a[7:4] = 4'hf;
    assign a[3:0] = drv;

    initial begin
        drv = 4'ha;
        #1;
        $display("CHECK: a=%h b=%h c=%h", a, b, c);
        drv = 4'h5;
        #1;
        $display("CHECK: a=%h b=%h c=%h", a, b, c);
        drv = 4'h3;
        #1;
        $display("CHECK: a=%h b=%h c=%h", a, b, c);
        $finish(0);
    end
endmodule
