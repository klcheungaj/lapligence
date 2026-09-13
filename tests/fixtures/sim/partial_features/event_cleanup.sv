module tb;
    event ev;
    logic clk=0,en=0;
    int count;
    always @(ev iff en or ev iff !en) count=count+1;
    initial begin
        fork
            begin @(posedge (clk && en) or ev iff en); $display("unexpected"); end
        join_none
        #1 ->ev;
        #1 disable fork;
        en=1; clk=1; ->ev;
        #1 ->ev;
        #1 $display("%0d",count);
        $finish(0);
    end
endmodule
