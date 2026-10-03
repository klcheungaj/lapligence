// llg-test-fixture: R05 port net-type collapse / aliases
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module and_leaf(inout triand [3:0] p, input wire [3:0] d);
    assign p = d;
endmodule
module plain(inout tri [3:0] p); endmodule
module tb;
    wire [3:0] a;
    wire [0:3] b;
    reg [3:0] d, other;
    alias a = b;
    // Reorder two pairs of electrical bits across a concat port connection.
    and_leaf u0({a[1:0], a[3:2]}, d);
    plain u1(b);
    assign b = other;
    initial begin
        other = 4'ha; d = 4'h7;
        #1 $display("aliases=%b/%b/%b", a, b, u0.p);
        d = 4'hf;
        #1 $display("changed=%b/%b/%b", a, b, u0.p);
        // Opposite known drivers must resolve to zero on every connected bit.
        other = 4'h0; d = 4'hf;
        #1 $display("zero_parent=%b/%b/%b", a, b, u0.p);
        other = 4'hf; d = 4'h0;
        #1 $display("zero_child=%b/%b/%b", a, b, u0.p);
        d = 4'hz; other = 4'hz;
        #1 $display("float=%b/%b/%b", a, b, u0.p);
        $finish(0);
    end
endmodule
