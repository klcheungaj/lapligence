module tb;
    typedef struct { int key; logic [7:0] data; } short_t;
    typedef struct { int key; logic [7:0] data; int tail; } long_t;
    typedef union { long_t extended; short_t compact; } choice_t;
    typedef struct { choice_t choice; logic [7:0] flag; } wrapper_t;
    wrapper_t wrapped;
    choice_t value, result;
    choice_t choices[2];
    function automatic choice_t copy(input choice_t source);
        choice_t local_value;
        local_value = source;
        local_value.compact.key += 2;
        return local_value;
    endfunction
    task automatic update(ref choice_t target);
        target.compact.data = 8'ha5;
    endtask
    initial begin
        value.extended = '{key:7, data:8'h5a, tail:11};
        if (value.compact.key != 7 || value.compact.data != 8'h5a) $fatal(1,"common sequence");
        result = copy(value);
        choices[0] = result;
        update(choices[0]);
        wrapped = '{choice:value, flag:8'h12};
        update(wrapped.choice);
        if (wrapped.choice.compact.key != 7 || wrapped.choice.compact.data != 8'ha5 || wrapped.flag != 8'h12)
            $fatal(1,"nested union view");
        $display("source=%0d,%h result=%0d,%h,%0d", value.compact.key, value.compact.data,
            choices[0].extended.key, choices[0].extended.data, choices[0].extended.tail);
        $finish(0);
    end
endmodule
