module tb;
    task automatic later(ref integer value);
        #1;
        value = value + 10;
    endtask
    task automatic exercise(input integer seed);
        integer value = seed;
        later(value);
        fork
            begin #1; $display("capture %0d", value); end
        join
        $display("joined %0d", value);
        fork
            begin
                #1;
                $display("detached %0d", value);
            end
        join_none
    endtask
    initial begin
        exercise(1);
        #2;
        exercise(2);
        #2;
        $finish(0);
    end
endmodule
