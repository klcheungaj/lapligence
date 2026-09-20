// Fixed-array dispatch must not intercept the existing resizable-container path.
module tb;
    int dynamic_values[];
    int queue_values[$];
    int associative_values[int];
    initial begin
        dynamic_values = new[3];
        dynamic_values[0] = 1; dynamic_values[1] = 2; dynamic_values[2] = 3;
        queue_values = '{2, 3, 4};
        associative_values[-1] = 7; associative_values[5] = 6;
        $display("dynamic=%0d queue=%0d assoc=%0d mapped=%0d", dynamic_values.sum(),
                 queue_values.product(), associative_values.and(), dynamic_values.sum() with (item + 1));
    end
endmodule
