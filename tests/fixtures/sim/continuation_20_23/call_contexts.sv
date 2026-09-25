// llg-test-fixture: SYN-013 caller capture, reference identity and activation lifetimes.
package call_types;
    typedef logic signed [64:0] word_t;
    typedef word_t row_t[-1:0];
    typedef struct { row_t data; bit valid; } record_t;
    function automatic record_t copied(input record_t value);
        return value;
    endfunction
endpackage
interface call_port;
    import call_types::*;
    function automatic word_t first(const ref record_t value);
        return value.data[-1];
    endfunction
endinterface
module tb;
    import call_types::*;
    call_port port_view();
    record_t values[2], snapshot;
    int index, calls, result;

    function automatic int choose();
        calls++;
        return index;
    endfunction
    task automatic alter(ref record_t value, const ref record_t alias_value,
                         output record_t result_value);
        value.data[-1] = word_t'(-2);
        if (alias_value.data[-1] !== word_t'(-2)) $fatal(1, "reference became a copy");
        result_value = value;
        value.data[0] = word_t'(9);
    endtask
    task automatic forward(ref record_t value, const ref record_t alias_value,
                           output record_t result_value);
        alter(value, alias_value, result_value);
    endtask
    task automatic copy_back(inout record_t value, output record_t result_value);
        value.data[0] = word_t'(17);
        index = 1;
        begin : local_exit
            result_value = value;
            disable local_exit;
            result_value.data[0] = word_t'(99);
        end
    endtask
    function automatic int activations(input int seed, input int extra = 3);
        int temporary = seed;
        static int count = 0;
        count++;
        begin : selected_exit
            if (seed == 0) disable selected_exit;
            temporary += extra;
        end
        return count * 100 + temporary;
    endfunction

    initial begin
        index = 0;
        calls = 0;
        values[0] = '{'{word_t'(1), word_t'(2)}, 1'b1};
        values[1] = '{'{word_t'(3), word_t'(4)}, 1'b0};
        forward(values[0], values[0], snapshot);
        if (values[0].data[0] !== word_t'(9) || snapshot.data[0] !== word_t'(2))
            $fatal(1, "reference write or output snapshot");
        if (port_view.first(values[0]) !== word_t'(-2)) $fatal(1, "interface const ref");
        snapshot = call_types::copied(values[0]);
        if (snapshot !== values[0]) $fatal(1, "package return");
        copy_back(values[choose()], snapshot);
        if (values[0].data[0] !== word_t'(17) || values[1].data[0] !== word_t'(4) ||
            snapshot.data[0] !== word_t'(17) || calls != 1) $fatal(1, "copy-back destination");
        result = activations(.seed(5));
        if (result != 108) $fatal(1, "automatic local initial value");
        result = activations(.extra(9), .seed(0));
        if (result != 200) $fatal(1, "static local and local disable");
        result = activations(2, 1);
        if (result != 303) $fatal(1, "fresh automatic activation");
        $display("CALL_CONTEXTS_PASS");
        $finish(0);
    end
endmodule
