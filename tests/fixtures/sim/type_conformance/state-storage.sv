module tb;
    typedef bit [128:0] two_t;
    typedef logic [128:0] four_t;
    typedef struct packed { bit [64:0] b; logic [63:0] l; } mixed_t;
    typedef struct { two_t b; four_t l; } unpacked_t;
    typedef union packed { two_t whole; bit [128:0] overlay; } union_t;
    mixed_t mixed_value;
    unpacked_t fields;
    union_t union_value;
    two_t fixed_two[0:1];
    four_t fixed_four[0:1];
    two_t dynamic_two[];
    four_t dynamic_four[];
    two_t queue_two[$];
    four_t queue_four[$];
    two_t keyed_two[int];
    four_t keyed_four[string];
    four_t source, expected, observed;
    initial begin
        source='0; source[128]=1; source[65]=1'bx; source[64]=1'bz;
        source[63]=1; source[2]=1'bx; source[1]=1'bz; source[0]=1;
        expected='0; expected[128]=1; expected[63]=1; expected[0]=1;
        if (fixed_two[0] !== '0 || fixed_four[1] !== 'x ||
            fields.b !== '0 || fields.l !== 'x || union_value !== '0) $display("FAIL initial storage");
        fixed_two[0]=source; fixed_four[0]=source;
        fixed_two[1]<=source; fixed_four[1]<=source;
        dynamic_two=new[2]; dynamic_four=new[2];
        if (dynamic_two[1] !== '0 || dynamic_four[1] !== 'x) $display("FAIL dynamic defaults");
        dynamic_two[0]=source; dynamic_four[0]=source;
        queue_two.push_back(source); queue_four.push_back(source);
        keyed_two[3]=source; keyed_four["key"]=source;
        fields.b=source; fields.l=source; union_value=source;
        mixed_value=source;
        if (mixed_value !== source || mixed_value.b !== expected[128:64]) $display("FAIL mixed member read");
        mixed_value.b=source[128:64];
        if (mixed_value !== {expected[128:64], source[63:0]}) $display("FAIL mixed member write");
        #1;
        if (fixed_two[0] !== expected || fixed_two[1] !== expected ||
            fixed_four[0] !== source || fixed_four[1] !== source ||
            dynamic_two[0] !== expected || dynamic_four[0] !== source ||
            queue_two[0] !== expected || queue_four[0] !== source ||
            keyed_two[3] !== expected || keyed_four["key"] !== source ||
            fields.b !== expected || fields.l !== source || union_value !== expected) $display("FAIL storage conversion");
        if (fixed_two[-1] !== '0 || fixed_four[-1] !== 'x ||
            dynamic_two[8] !== '0 || dynamic_four[8] !== 'x ||
            queue_two[8] !== '0 || queue_four[8] !== 'x ||
            keyed_two[8] !== '0 || keyed_four["absent"] !== 'x) $display("FAIL missing defaults");
        fixed_two[-1]=source; fixed_four[2]=0;
        dynamic_two[-1]=0; dynamic_four[2]=0;
        if (fixed_two[0] !== expected || fixed_four[0] !== source ||
            dynamic_two[0] !== expected || dynamic_four[0] !== source) $display("FAIL invalid write");
        observed=queue_two.pop_front();
        if (observed !== expected) $display("FAIL two-state pop");
        observed=queue_four.pop_front();
        if (observed !== source) $display("FAIL four-state pop");
        dynamic_two=new[3](dynamic_two); dynamic_four=new[3](dynamic_four);
        if (dynamic_two[0] !== expected || dynamic_two[2] !== '0 ||
            dynamic_four[0] !== source || dynamic_four[2] !== 'x) $display("FAIL resize");
        dynamic_two.delete(); dynamic_four.delete(); keyed_two.delete(); keyed_four.delete();
        if (dynamic_two.size() !== 0 || dynamic_four.size() !== 0 ||
            keyed_two.num() !== 0 || keyed_four.num() !== 0) $display("FAIL deletion");
        $display("PASS state storage"); $finish(0);
    end
endmodule
