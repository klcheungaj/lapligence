module child(ref logic [3:0] a);
    initial begin
        $display("initial %h",a);
        #1 a[0]=1;
    end
endmodule
module tb;
    logic [3:0] a=4'ha;
    child c(a);
    initial begin
        @(posedge a);
        $display("edge %0t %h %h",$time,a,c.a);
        $finish(0);
    end
endmodule
