module tb;
    logic clk,en;
    int count;
    always @(posedge clk iff en) count=count+1;
    initial begin
        clk=0; en=0;
        // The qualifier is read when the edge is seen, at the check of the
        // writing process (T1-D11), so a write in the same turn counts.
        #1 clk=1; en=1;
        #1 $display("set with the edge %0d",count);
        clk=0;
        #1 clk=1; en=0;
        #1 $display("cleared with the edge %0d",count);
        clk=0; en=1'bx;
        #1 clk=1;
        #1 $display("unknown %0d",count);
        $finish(0);
    end
endmodule
