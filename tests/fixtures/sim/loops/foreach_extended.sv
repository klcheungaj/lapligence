// llg-test-fixture: tests/fixtures/sim/loops/foreach_extended.sv
// IEEE 1800-2009 §§6.21, 12.7.1, and 12.7.3: omitted foreach dimensions are skipped;
// dynamic arrays, queues, and associative arrays enumerate their current
// dimensions/keys in declaration or key order.
module tb;
    int fixed [2:1][3:5];
    int dynamic_array[];
    int queue[$];
    int associative[int];
    logic [7:0] string_associative[string];
    integer omitted_first;
    integer omitted_last;
    integer all_omitted;
    integer dynamic_sum;
    integer queue_sum;
    integer associative_sum;
    integer string_associative_sum;
    integer count;
    real real_sum;
    real real_shadow;
    real real_control;
    real capture_sum;

    function automatic integer function_foreach_sum;
        begin
            function_foreach_sum = 0;
            foreach (fixed[function_index]) begin
                function_foreach_sum += function_index;
            end
        end
    endfunction

    initial begin
        fixed = '{default: 0};
        fixed[1][3] = 13;
        fixed[1][4] = 14;
        fixed[1][5] = 15;
        fixed[2][3] = 23;
        fixed[2][4] = 24;
        fixed[2][5] = 25;

        omitted_first = 0;
        count = 0;
        foreach (fixed[, j]) begin
            omitted_first = omitted_first * 10 + j;
            count++;
        end

        omitted_last = 0;
        foreach (fixed[i,]) begin
            omitted_last = omitted_last * 10 + i;
        end

        all_omitted = 17;
        foreach (fixed[,]) begin
            all_omitted++;
        end

        dynamic_array = new[3];
        dynamic_array[0] = 4;
        dynamic_array[1] = 5;
        dynamic_array[2] = 6;
        dynamic_sum = 0;
        foreach (dynamic_array[i]) begin
            dynamic_sum += dynamic_array[i];
        end

        queue.push_back(7);
        queue.push_back(8);
        queue.push_back(9);
        queue_sum = 0;
        foreach (queue[i]) begin
            queue_sum += queue[i];
        end

        associative[9] = 90;
        associative[1] = 10;
        associative[5] = 50;
        associative_sum = 0;
        foreach (associative[k]) begin
            associative_sum = associative_sum * 10 + k;
            associative.delete(k);
        end

        string_associative["gamma"] = 3;
        string_associative["alpha"] = 1;
        string_associative["beta"] = 2;
        string_associative_sum = 0;
        foreach (string_associative[string_key]) begin
            string_associative_sum = string_associative_sum * 10 + string_associative[string_key];
            string_associative.delete(string_key);
        end

        real_sum = 0.0;
        for (real r = 0.5; r < 2.0; r = r + 0.5) begin
            real_sum += r;
        end
        real_shadow = 0.0;
        for (real r = 10.0; r < 12.0; r = r + 1.0) begin : real_outer
            real_shadow += r;
            begin : real_inner
                real r;
                r = 100.0;
                real_shadow += r;
            end
        end
        real_control = 0.0;
        for (real c = 0.0; c < 4.0; c = c + 1.0) begin
            if (c == 1.0) continue;
            if (c == 3.0) break;
            real_control += c;
        end
        capture_sum = 0.0;
        for (real captured = 1.0; captured < 3.0; captured = captured + 1.0) begin
            fork
                capture_sum = capture_sum + captured;
            join
        end

        $display("first=%0d count=%0d last=%0d omitted=%0d dynamic=%0d queue=%0d assoc=%0d string_assoc=%0d remaining=%0d real=%0f shadow=%0f control=%0f fn=%0d capture=%0f",
                 omitted_first, count, omitted_last, all_omitted, dynamic_sum, queue_sum,
                 associative_sum, string_associative_sum, associative.num(), real_sum,
                 real_shadow, real_control, function_foreach_sum(), capture_sum);
        $finish;
    end
endmodule
