module tb;
    event events[2:1];
    int index;
    logic [31:0] unknown_index;
    initial begin
        index = 1;
        @(events[index]);
        $display("event");
        $finish(0);
    end
    initial begin
        -> events[unknown_index];
        if (events[unknown_index].triggered)
            $display("unexpected unknown-index trigger");
    end
    initial begin
        #1;
        -> events[1];
    end
endmodule
