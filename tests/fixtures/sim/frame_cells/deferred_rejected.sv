module tb;
    task automatic report;
        integer local_value = 1;
        $monitor("%0d", local_value);
        $strobe("%0d", local_value);
    endtask
    initial report();
endmodule
