module tb;
    integer completed = 0;
    initial begin
        begin : victim
            #5;
            $display("WRONG: cancelled body resumed");
        end
        completed = completed + 1;
    end
    initial begin
        #1;
        disable victim;
        #1;
        $display("continued=%0d", completed);
        #5;
        $display("still=%0d", completed);
        $finish(0);
    end
endmodule
