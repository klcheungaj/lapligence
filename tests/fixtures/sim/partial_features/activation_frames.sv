// IEEE 1800-2009 6.21, 9.3, 12.7, and 13.3: a join_none branch shares the
// automatic variables it references with the enclosing activation, which
// outlives the returning subroutine. A for-loop variable is one variable per
// loop execution, so every branch prints its final value (3, 2 and 12).
module tb;
    integer first;
    integer second;

    function automatic integer bump(input integer value);
        integer local_value;
        local_value = value + 1;
        return local_value;
    endfunction

    function automatic integer spawn_capture(input integer value);
        integer local_value;
        local_value = value + 100;
        fork
            begin
                #1 $display("function formal %0d", value);
            end
            begin
                #1 $display("function capture %0d", local_value);
            end
        join_none
        return local_value;
    endfunction

    task automatic spawn_task(input integer value);
        integer local_value;
        local_value = value + 200;
        fork
            begin
                #1 $display("task capture %0d", local_value);
            end
        join_none
    endtask

    initial begin
        first = bump(4);
        second = bump(9);
        first = spawn_capture(first);
        second = spawn_capture(second);
        spawn_task(1);
        for (int index = 0; index < 3; index = index + 1) begin
            fork
                begin
                    #1 $display("capture %0d", index);
                end
            join_none
        end
        for (int index = 0; index < 2; index = index + 1) begin
            fork
                begin
                    #1 $display("outer %0d", index);
                end
            join_none
            for (int index = 10; index < 12; index = index + 1) begin
                fork
                    begin
                        #1 $display("inner %0d", index);
                    end
                join_none
            end
        end
        #2;
        wait fork;
        if (first !== 105 || second !== 110)
            $display("FAIL activation_frames %0d %0d", first, second);
        else
            $display("PASS activation_frames");
        $finish(0);
    end
endmodule
