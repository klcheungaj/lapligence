// IEEE 1800-2009 7.10: queues begin empty and preserve order across indexed
// access, insertion, deletion, and front/back push/pop operations.
module tb;
    integer values[$];
    integer popped;

    initial begin
        if (values.size() !== 0) begin
            $display("FAIL queue default_size");
            $finish;
        end

        values.push_back(2);
        values.push_front(1);
        values.push_back(3);
        if (values.size() !== 3 || values[0] !== 1 ||
            values[1] !== 2 || values[2] !== 3) begin
            $display("FAIL queue push_order");
            $finish;
        end

        popped = values.pop_front();
        if (popped !== 1 || values.size() !== 2 || values[0] !== 2) begin
            $display("FAIL queue pop_front");
            $finish;
        end
        popped = values.pop_back();
        if (popped !== 3 || values.size() !== 1 || values[0] !== 2) begin
            $display("FAIL queue pop_back");
            $finish;
        end

        values.insert(0, 9);
        if (values.size() !== 2 || values[0] !== 9 || values[1] !== 2) begin
            $display("FAIL queue insert");
            $finish;
        end
        values.delete(1);
        if (values.size() !== 1 || values[0] !== 9) begin
            $display("FAIL queue element_delete");
            $finish;
        end
        values.delete();
        if (values.size() !== 0) begin
            $display("FAIL queue whole_delete");
            $finish;
        end

        $display("PASS queue");
        $finish;
    end
endmodule
