module tb;
    task automatic update;
        integer local_value = 1;
        local_value <= #1 2;
        return;
    endtask
    initial update();
endmodule
