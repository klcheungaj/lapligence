// llg-test-fixture: IEEE 1800-2009 23.3.3.7. A parent driver and a child
// inout driver are members of ONE resolved net, so simultaneous opposite
// drives resolve to X instead of one directional link overwriting the other.
module drv(inout wire p, input logic en, input logic d);
    assign p = en ? d : 1'bz;
endmodule

module tb;
    wire bus;
    logic pen, pd;
    logic cen, cd;
    assign bus = pen ? pd : 1'bz;
    drv u(.p(bus), .en(cen), .d(cd));

    initial begin
        pen = 1'b0; pd = 1'b0;
        cen = 1'b0; cd = 1'b0;
        #1 $display("none=%b", bus);
        pen = 1'b1; pd = 1'b1;
        #1 $display("parent=%b", bus);
        pen = 1'b0; cen = 1'b1; cd = 1'b0;
        #1 $display("child=%b", bus);
        pen = 1'b1; pd = 1'b1; cen = 1'b1; cd = 1'b0;
        #1 $display("conflict=%b", bus);
        pen = 1'b0; cen = 1'b0;
        #1 $display("release=%b", bus);
        $finish(0);
    end
endmodule
