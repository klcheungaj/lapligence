// IEEE 1800-2009 7.8, 7.9, and 7.10: descriptor-backed queues and
// associative arrays own real and string values across mutation, copying,
// defaults, and integral or string-key traversal.
module tb;
    real real_queue[$];
    real real_queue_copy[$];
    real bounded_real_queue[$:2];
    string string_queue[$];
    string string_queue_copy[$];
    chandle handle_queue[$];
    real real_values[int];
    real real_values_copy[int];
    string string_values[string];
    string string_values_copy[string];
    int int_key;
    string string_key;
    integer status;
    integer real_changes = 0;
    real nan_value;

    always @(real_queue[0]) real_changes = real_changes + 1;

    initial begin
        real_queue[real_queue.size()] = -0.0;
        string_queue[string_queue.size()] = "append";
        handle_queue[handle_queue.size()] = null;
        if (real_queue.size() !== 1 || string_queue.size() !== 1 ||
            handle_queue.size() !== 1 ||
            $realtobits(real_queue[0]) !== 64'h8000000000000000 ||
            string_queue[0] != "append" || handle_queue[0] != null) begin
            $display("FAIL generic_containers end_index_append");
            $finish;
        end

        real_queue = '{0.0};
        #1 real_changes = 0;
        real_queue[0] = -0.0;
        #1;
        if ($realtobits(real_queue[0]) !== 64'h8000000000000000 ||
            real_changes !== 1) begin
            $display("FAIL generic_containers signed_zero_change");
            $finish;
        end
        nan_value = $bitstoreal(64'h7ff8000000000001);
        real_queue[0] = nan_value;
        #1 real_changes = 0;
        real_queue[0] = nan_value;
        #1;
        if (real_changes !== 0) begin
            $display("FAIL generic_containers repeated_nan_change");
            $finish;
        end

        real_queue = '{1.5, 2.5};
        real_queue.push_back(3.5);
        real_queue.push_front(-1.0);
        real_queue.insert(2, 9.5);
        real_queue_copy = real_queue;
        real_queue[0] = 7.5;
        if (real_queue.size() !== 5 || real_queue[$] != 3.5 ||
            real_queue_copy[0] != -1.0 || real_queue_copy[2] != 9.5) begin
            $display("FAIL generic_containers real_queue_copy");
            $finish;
        end
        real_queue.delete(1);
        if (real_queue.size() !== 4 || real_queue[0] != 7.5 ||
            real_queue[1] != 9.5) begin
            $display("FAIL generic_containers real_queue_delete");
            $finish;
        end

        bounded_real_queue = '{1.0, 2.0, 3.0, 4.0};
        bounded_real_queue.push_front(0.0);
        bounded_real_queue.push_back(5.0);
        if (bounded_real_queue.size() !== 3 || bounded_real_queue[0] != 0.0 ||
            bounded_real_queue[2] != 2.0) begin
            $display("FAIL generic_containers bounded_real_queue");
            $finish;
        end

        string_queue = '{"one", "two"};
        string_queue.push_front("zero");
        string_queue.push_back("three");
        string_queue.insert(2, "middle");
        string_queue_copy = string_queue;
        string_queue[0] = "changed";
        if (string_queue.size() !== 5 || string_queue[0].len() !== 7 ||
            string_queue[$].len() !== 5 || string_queue_copy[0].len() !== 4 ||
            string_queue_copy[2].len() !== 6) begin
            $display("FAIL generic_containers string_queue_copy");
            $finish;
        end

        real_values = '{default: 1.25, -2: -2.5, 4: 4.5};
        if (real_values.num() !== 2 || real_values[99] != 1.25 ||
            real_values[-2] != -2.5 || real_values[4] != 4.5) begin
            $display("FAIL generic_containers real_assoc_default");
            $finish;
        end
        status = real_values.first(int_key);
        if (status !== 1 || int_key !== -2) begin
            $display("FAIL generic_containers real_assoc_first");
            $finish;
        end
        status = real_values.next(int_key);
        if (status !== 1 || int_key !== 4) begin
            $display("FAIL generic_containers real_assoc_next");
            $finish;
        end
        status = real_values.last(int_key);
        if (status !== 1 || int_key !== 4) begin
            $display("FAIL generic_containers real_assoc_last");
            $finish;
        end
        status = real_values.prev(int_key);
        if (status !== 1 || int_key !== -2) begin
            $display("FAIL generic_containers real_assoc_prev");
            $finish;
        end
        real_values_copy = real_values;
        real_values.delete(4);
        if (real_values.num() !== 1 || real_values_copy.num() !== 2 ||
            real_values_copy[4] != 4.5) begin
            $display("FAIL generic_containers real_assoc_copy");
            $finish;
        end

        string_values = '{default: "missing"};
        string_values["z"] = "last";
        string_values["a"] = "first";
        if (string_values.num() !== 2 || string_values["missing"].len() !== 7 ||
            string_values["z"].len() !== 4 || string_values["a"].len() !== 5) begin
            $display("FAIL generic_containers string_assoc_default");
            $finish;
        end
        status = string_values.first(string_key);
        if (status !== 1 || string_key != "a") begin
            $display("FAIL generic_containers string_assoc_first");
            $finish;
        end
        status = string_values.next(string_key);
        if (status !== 1 || string_key != "z") begin
            $display("FAIL generic_containers string_assoc_next");
            $finish;
        end
        status = string_values.last(string_key);
        if (status !== 1 || string_key != "z") begin
            $display("FAIL generic_containers string_assoc_last");
            $finish;
        end
        string_values_copy = string_values;
        string_values.delete("a");
        if (string_values.num() !== 1 || string_values_copy.num() !== 2 ||
            string_values_copy["a"].len() !== 5) begin
            $display("FAIL generic_containers string_assoc_copy");
            $finish;
        end

        $display("PASS generic_containers_p32_p33");
        $finish;
    end
endmodule
