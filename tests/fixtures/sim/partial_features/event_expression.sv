module tb;
    logic a=0,b=0;
    int changes, rises;
    always @(a && b) changes=changes+1;
    always @(posedge (a && b)) rises=rises+1;
    initial begin
        a=0; b=0;
        #1 a=1;
        #1 $display("same %0d %0d",changes,rises);
        b=1;
        #1 $display("rise %0d %0d",changes,rises);
        a=0;
        #1 $display("fall %0d %0d",changes,rises);
        b=0;
        #1 $display("same %0d %0d",changes,rises);
        $finish;
    end
endmodule
