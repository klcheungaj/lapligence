module tb;
    logic [3:0] a=0;
    logic enable=1;
    int rises,falls,changes;
    always @(posedge a[1] iff enable) rises=rises+1;
    always @(negedge a[1] iff enable) falls=falls+1;
    always @(a[1]) changes=changes+1;
    initial begin
        #1 a[3]=1;
        #1 a[1]=1'bx;
        #1 a[1]=1'bz;
        #1 a[1]=1;
        #1 a[1]=1'bz;
        #1 a[1]=0;
        #1 $display("%0d %0d %0d",rises,falls,changes);
        $finish(0);
    end
endmodule
