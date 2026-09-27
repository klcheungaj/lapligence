// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/static_function_array_nba.sv
// IEEE 1800-2009 §§10.4.2 and 13.4.4 permit a function to schedule
// an NBA to persistent storage when called from an initial process.
module tb;
    logic [7:0] old_static;
    logic [7:0] old_explicit;

    function static logic [7:0] put(input logic [7:0] value);
        logic [7:0] data [0:1];
        data[0] <= value;
        put = data[0];
    endfunction

    function automatic logic [7:0] poke(input logic [7:0] value);
        static logic [7:0] data [0:1];
        data[0] <= value;
        poke = data[0];
    endfunction

    initial begin
        old_static = put(8'h35);
        old_explicit = poke(8'h53);
        #1;
        if (put.data[0] !== 8'h35 || poke.data[0] !== 8'h53)
            $fatal(1, "function NBA did not publish after return");
        old_static = put(8'h46);
        old_explicit = poke(8'h64);
        #1;
        if (old_static !== 8'h35 || old_explicit !== 8'h53 ||
            put.data[0] !== 8'h46 || poke.data[0] !== 8'h64)
            $fatal(1, "function static array or RHS capture mismatch");
        $display("static=%h explicit=%h old=%h/%h",
                 put.data[0], poke.data[0], old_static, old_explicit);
        $finish(0);
    end
endmodule
