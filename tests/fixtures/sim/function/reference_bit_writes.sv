module tb;
    logic [7:0] value;
    logic [4:7] ascending;
    bit [7:0] two_state;
    logic [7:0] queue_values[$];
    integer selector_calls;
    logic old_bit;

    function automatic int next_bit();
        selector_calls = selector_calls + 1;
        return 2;
    endfunction

    function automatic void update(ref logic [7:0] target);
        target[2] = 1;
        target[1] = 1'bx;
        target[9] = 1;
        target[3'bxxx] = 1;
        old_bit = target[next_bit()]++;
    endfunction

    function automatic void update_ascending(ref logic [4:7] target);
        target[5] = 1;
    endfunction

    function automatic void update_two_state(ref bit [7:0] target);
        target[3] = 1'bx;
    endfunction

    function automatic void update_queue(ref logic [7:0] target);
        queue_values.push_front(8'h01);
        target[2] = 1;
        $display("retained=%h", target);
    endfunction

    initial begin
        value = 0;
        selector_calls = 0;
        update(value);
        $display("value=%b old=%b calls=%0d", value, old_bit, selector_calls);
        ascending = 0;
        update_ascending(ascending);
        $display("ascending=%b", ascending);
        two_state = '1;
        update_two_state(two_state);
        $display("two-state=%h", two_state);
        queue_values.push_back(8'h80);
        update_queue(queue_values[0]);
        $display("queue=%h,%h", queue_values[0], queue_values[1]);
        $finish(0);
    end
endmodule
