// IEEE 1800-2009 7.8, 7.9, and 7.10: recursive container elements retain
// owned child arrays across queue and associative insertion, nested updates,
// copying, and structural deletion.
module tb;
    int child[];
    int second[];
    int nested_queue[$][];
    int nested_queue_copy[$][];
    int nested_values[int][];
    int nested_values_copy[int][];
    string words[];
    string nested_strings[$][];

    initial begin
        child = new[2];
        child[0] = 10;
        child[1] = 20;
        second = new[1];
        second[0] = 30;

        nested_queue.push_back(child);
        nested_queue.insert(1, second);
        nested_queue[0][1] = 21;
        nested_queue_copy = nested_queue;
        nested_queue[0][0] = 11;
        child[0] = 99;
        if (nested_queue.size() !== 2 || nested_queue[0][0] !== 11 ||
            nested_queue[0][1] !== 21 || nested_queue[1][0] !== 30 ||
            nested_queue_copy[0][0] !== 10 || nested_queue_copy[0][1] !== 21 ||
            child[0] !== 99) begin
            $display("FAIL nested_container_values queue_copy");
            $finish;
        end
        nested_queue.delete(0);
        if (nested_queue.size() !== 1 || nested_queue[0][0] !== 30) begin
            $display("FAIL nested_container_values queue_delete");
            $finish;
        end

        nested_values[3] = child;
        nested_values[3][1] = 31;
        nested_values[7] = second;
        nested_values_copy = nested_values;
        nested_values[3][0] = 32;
        if (nested_values.num() !== 2 || nested_values[3][0] !== 32 ||
            nested_values[3][1] !== 31 || nested_values[7][0] !== 30 ||
            nested_values_copy[3][0] !== 99 || nested_values_copy[3][1] !== 31) begin
            $display("FAIL nested_container_values assoc_copy");
            $finish;
        end
        nested_values.delete(3);
        if (nested_values.num() !== 1 || !nested_values.exists(7)) begin
            $display("FAIL nested_container_values assoc_delete");
            $finish;
        end

        words = new[2];
        words[0] = "alpha";
        words[1] = "beta";
        nested_strings.push_back(words);
        nested_strings[0][1] = "changed";
        if (nested_strings[0][0].len() !== 5 ||
            nested_strings[0][1].len() !== 7) begin
            $display("FAIL nested_container_values string_nested");
            $finish;
        end

        $display("PASS nested_container_values_p32_p33");
        $finish;
    end
endmodule
