// llg-test-fixture: R13 record deconstruction into a selected NBA destination.
// R13 composition: structure deconstruction captures a selected NBA target and source.
// IEEE 1800-2009 10.9 and 10.4.2.
module tb;
    typedef struct packed {
        logic [7:0] hi;
        logic [7:0] lo;
    } pair_t;

    pair_t source;
    logic [7:0] selected [0:1];
    logic [7:0] tail;
    int destination;

    initial begin
        selected[0] = 8'ha0;
        selected[1] = 8'hb0;
        tail = 8'hc0;
        destination = 1;
        source = '{hi: 8'h12, lo: 8'h34};

        pair_t'{selected[destination], tail} <= source;
        destination = 0;
        source = '{hi: 8'he1, lo: 8'he2};

        #1;
        if (selected[0] !== 8'ha0 || selected[1] !== 8'h12 || tail !== 8'h34)
            $fatal(1, "record pattern NBA target/source capture");
        $display("selected record NBA scatter passed: %h/%h", selected[1], tail);
        $finish(0);
    end
endmodule
