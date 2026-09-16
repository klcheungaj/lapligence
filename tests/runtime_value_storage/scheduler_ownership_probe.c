// Include the facade to exercise private commit/cancel paths without relying
// on coroutine timing. The separate coroutine probe tests real suspension.
#include "llg_rt.c"
#include "probe.h"

static void never_run(llg_proc_t* proc) { (void)proc; abort(); }

static void check_file_messages(void) {
    char text[160];
    char large[200];
    memset(large, 'x', sizeof(large));
    large[sizeof(large) - 1u] = 0;
    llg_file_set_message(text, sizeof(text), large);
    CHECK(strlen(text) == sizeof(text) - 1u);
    CHECK(text[0] == 'x' && text[sizeof(text) - 2u] == 'x');
    llg_file_set_message(text, sizeof(text), text);
    CHECK(strlen(text) == sizeof(text) - 1u);
    llg_file_set_message(text, sizeof(text), "prefix: diagnostic");
    llg_file_set_message(text, sizeof(text), text + 8);
    CHECK(strcmp(text, "diagnostic") == 0);
    llg_file_set_message(text, sizeof(text), NULL);
    CHECK(text[0] == 0);
    text[1] = 's';
    llg_file_set_message(text, 1, large);
    CHECK(text[0] == 0 && text[1] == 's');
    llg_file_set_message(text, 0, large);
    CHECK(text[0] == 0 && text[1] == 's');
}

static void check_nba_and_scopes(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t target = sv4_zero(129, 0);
    sv4_t source = sv4_from_u64(42, 129, 0);
    sv4_t mask = sv4_from_u64(255, 129, 0);
    const size_t baseline = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        source.bits[0] = 42;
        llg_nba_after(&target, source, 0);
        CHECK(g.delayed_nbas->value.bits != source.bits);
        source.bits[0] = 99;
        commit_nbas(LLG_REGION_NBA);
        CHECK(target.bits[0] == 42);
        CHECK(value_test_live() == baseline);
        llg_nba_masked(&target, source, mask, 1);
        source.bits[0] = 7;
        ++g.now;
        commit_nbas(LLG_REGION_NBA);
        CHECK(target.bits[0] == 99);
        CHECK(value_test_live() == baseline);
    }
    llg_proc_t* proc = llg_spawn(never_run, "cancel-owner");
    // Supply current-process identity without switching the C stack.
    aco_gtls_co = proc->co;
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* values = llg_value_scope_values(scope);
    sv4_replace(&values[0], sv4_clone(&source));
    sv4_replace(&values[1], sv4_zero(65537, 0));
    llg_nba_masked(&target, source, mask, 0);
    CHECK(proc->nba_head != NULL && g.delayed_nbas == NULL);
    proc->wait.kind = W_EVENTS;
    proc->wait.n = 2;
    proc->wait.last = llg_checked_calloc(2, sizeof(sv4_t), "test wait snapshots");
    sv4_copy(&proc->wait.last[0], &source);
    sv4_copy(&proc->wait.last[1], &target);
    sv4_copy(&proc->wait.level_val, &source);
    proc->wait.next = g.waiters;
    g.waiters = &proc->wait;
    ++g.wait_count;
    aco_gtls_co = g.main_co;
    llg_kill_proc(proc, 0);
    reap_retired_procs();
    CHECK(g.wait_count == 0 && g.waiters == NULL);
    CHECK(value_test_live() == baseline);
    scope = llg_value_scope_begin(1);
    sv4_replace(llg_value_scope_values(scope), sv4_clone(&source));
    llg_value_scope_end(scope);
    CHECK(value_test_live() == baseline);
    scope = llg_value_scope_begin(1);
    sv4_replace(llg_value_scope_values(scope), sv4_zero(65537, 0));
    llg_nba_after(&target, source, 100);
    llg_rt_cleanup();
    CHECK(value_test_live() == baseline);
    sv4_destroy(&source);
    sv4_destroy(&target);
    sv4_destroy(&mask);
    CHECK(value_test_live() == 0);
}

static void check_frames(void) {
    llg_rt_init();
    sv4_t source = sv4_from_u64(17, 65, 0);
    for (unsigned i = 0; i < 1000; ++i) {
        llg_frame_t* parent = llg_frame_new(2);
        llg_frame_t* child = llg_frame_new(1);
        source.bits[0] = 17;
        llg_frame_capture_value(parent, 0, source);
        llg_frame_capture_value(parent, 0, source);
        source.bits[0] = 22;
        expect_number(llg_frame_read_value(parent, 0), 17);
        llg_frame_alias_slot(child, 0, parent, 0);
        llg_frame_release(parent);
        llg_frame_write_value(child, 0, source);
        expect_number(llg_frame_read_value(child, 0), 22);
        llg_frame_release(child);
        CHECK(value_test_live() == 1);
    }
    llg_rt_cleanup();
    sv4_destroy(&source);
    CHECK(value_test_live() == 0);
}

static void check_inertial_and_force(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t target = sv4_zero(65, 0);
    sv4_t source = sv4_from_u64(1, 65, 0);
    sv4_t mask = sv4_from_u64(255, 65, 0);
    llg_inertial_t* driver = NULL;
    llg_inertial_assign(&driver, &target, source, 2, 2, 2);
    source.bits[0] = 0;
    llg_inertial_assign(&driver, &target, source, 2, 2, 2);
    CHECK(driver && !driver->pending && driver->value.bits == NULL);
    size_t retained = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        source.bits[0] = i % 2 ? 0xaa : 0x55;
        uint64_t expected = source.bits[0];
        llg_inertial_selected_assign(&driver, &target, source, mask, 2, 2, 2);
        source.bits[0] = 0;
        g.now += 2;
        commit_inertial(LLG_REGION_ACTIVE);
        CHECK(target.bits[0] == expected);
        CHECK(value_test_live() == retained);
        llg_force(&target, source);
        CHECK(target.bits[0] == 0);
        source.bits[0] = 19;
        llg_ba(&target, source);
        CHECK(target.bits[0] == 0);
        llg_release(&target);
        CHECK(value_test_live() == retained);
    }
    source.bits[0] = 1;
    llg_inertial_assign(&driver, &target, source, 100, 100, 100);
    llg_rt_cleanup();
    CHECK(driver == NULL && value_test_live() == 3);
    sv4_destroy(&source);
    sv4_destroy(&mask);
    sv4_destroy(&target);
    CHECK(value_test_live() == 0);
}

static void check_sequence_snapshots(void) {
    llg_rt_init();
    const llg_sequence_local_t locals[] = {{65, 0, 0, 1}, {129, 1, 1, 2}};
    const llg_sequence_graph_t graph = {.local_count = 2, .locals = locals};
    sv4_t source = sv4_from_u64(23, 65, 0);
    for (unsigned i = 0; i < 1000; ++i) {
        llg_sequence_attempt_t* attempt = sequence_attempt_new(&graph, 0, NULL, NULL);
        llg_sequence_local_write(&attempt->locals[0], source);
        llg_sequence_attempt_t* inherited = sequence_attempt_new(&graph, 0, &graph, attempt->locals);
        llg_sequence_token_t seed = {.locals = attempt->locals};
        llg_sequence_token_t* token = sequence_token_copy(&graph, &seed);
        sequence_endpoint_add(attempt, token, 0);
        attempt->locals[0].bits[0] = 99;
        expect_number(sv4_clone(&token->locals[0]), 23);
        expect_number(sv4_clone(&attempt->endpoints->locals[0]), 23);
        expect_number(llg_sequence_local_read(inherited, 0), 23);
        sequence_token_free(token);
        sequence_attempt_discard(inherited);
        sequence_attempt_discard(attempt);
        CHECK(value_test_live() == 1);
    }
    sv4_destroy(&source);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
}

static void check_sampling_mailboxes_and_reinit(void) {
    for (unsigned cycle = 0; cycle < 25; ++cycle) {
        llg_rt_init();
        g.current_region = LLG_REGION_ACTIVE;
        sv4_t source = sv4_from_u64(17, 65, 0);
        sv4_t target = sv4_zero(65, 0);
        sv4_t bound = sv4_zero(32, 0);
        llg_sampled_register(&source);
        sample_preponed_values();
        source.bits[0] = 33;
        sampled_record_write(&source);
        sv4_t snapshot = SV4_EMPTY;
        CHECK(llg_sampled_copy(&source, &snapshot));
        expect_number(sv4_clone(&snapshot), 17);
        sv4_destroy(&snapshot);
        llg_mailbox_t* mailbox = llg_mailbox_new(bound, LLG_MAILBOX_PACKED, 65, 0, 0, 0);
        size_t retained = value_test_live();
        for (unsigned i = 0; i < 100; ++i) {
            source.bits[0] = 33;
            CHECK(llg_mailbox_try_put_value(mailbox, llg_mailbox_value_packed(source, 65, 0, 0)));
            source.bits[0] = 99;
            CHECK(llg_mailbox_try_get_value(mailbox, llg_mailbox_target_packed(&target, 65, 0, 0), 1));
            CHECK(target.bits[0] == 33);
            CHECK(llg_mailbox_try_get_value(mailbox, llg_mailbox_target_packed(&target, 65, 0, 0), 0));
            CHECK(target.bits[0] == 33);
            CHECK(value_test_live() == retained);
        }
        CHECK(llg_mailbox_try_put_value(mailbox, llg_mailbox_value_packed(source, 65, 0, 0)));
        // init tears down the previous runtime even with pending retained owners.
        llg_rt_init();
        CHECK(value_test_live() == 3);
        llg_rt_cleanup();
        sv4_destroy(&source);
        sv4_destroy(&target);
        sv4_destroy(&bound);
        CHECK(value_test_live() == 0);
    }
}


static void check_time_and_io(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    // Exact integer multiplication/division oracle, seed 424243, half-up.
    static const uint64_t time_vectors[][4] = {
        {UINT64_C(0), UINT64_C(1), UINT64_C(1), UINT64_C(0)},
        {UINT64_C(1), UINT64_C(1), UINT64_C(2), UINT64_C(1)},
        {UINT64_C(1), UINT64_C(1), UINT64_C(3), UINT64_C(0)},
        {UINT64_C(18446744073709551615), UINT64_C(1), UINT64_C(1), UINT64_C(18446744073709551615)},
        {UINT64_C(4803863608740140353), UINT64_C(9676843720674511846), UINT64_C(17281984797905143529), UINT64_C(2689866814536760390)},
        {UINT64_C(4496948063835937765), UINT64_C(2707638318670392135), UINT64_C(12928502885552316538), UINT64_C(941803471175281417)},
        {UINT64_C(1768655087190374748), UINT64_C(7496775825935457702), UINT64_C(13602965696127084819), UINT64_C(974729408149695984)},
        {UINT64_C(8474865438317343514), UINT64_C(3574590434559897451), UINT64_C(11004297157256196698), UINT64_C(2752940282970782241)},
        {UINT64_C(5522339163427738794), UINT64_C(10551291666008678704), UINT64_C(15364225026150806765), UINT64_C(3792434118399934086)},
        {UINT64_C(13911469441618561882), UINT64_C(8806673106093756393), UINT64_C(10616459756555306210), UINT64_C(11539982876316108467)},
        {UINT64_C(3788481180487197648), UINT64_C(14813622448124458524), UINT64_C(5952117793502101889), UINT64_C(9428766668702249904)},
        {UINT64_C(6613758981624084093), UINT64_C(9002194832525891305), UINT64_C(14098655646530192607), UINT64_C(4222980433074195968)},
        {UINT64_C(1065858688303747038), UINT64_C(12736647595969134315), UINT64_C(8996735262702857022), UINT64_C(1508932529815077129)},
        {UINT64_C(17498516564740927368), UINT64_C(447790682552770724), UINT64_C(6919083719563251570), UINT64_C(1132472592293031297)},
        {UINT64_C(2875714667991991232), UINT64_C(464424255977221567), UINT64_C(6350975240094520141), UINT64_C(210290797018614507)},
        {UINT64_C(12860840049160111812), UINT64_C(16008573890739634048), UINT64_C(16758296066416900053), UINT64_C(12285479824917742946)},
        {UINT64_C(2278789545051072539), UINT64_C(14059301651726354164), UINT64_C(17230886766129893515), UINT64_C(1859346535643746232)},
        {UINT64_C(5598009744802737710), UINT64_C(3795805088974750705), UINT64_C(16926407781997336765), UINT64_C(1255372914981555220)},
        {UINT64_C(513993566496042085), UINT64_C(4317287544667640020), UINT64_C(4212018648156371159), UINT64_C(526839553201873284)},
        {UINT64_C(3320387462288190472), UINT64_C(12098200741337443356), UINT64_C(7553953646032332050), UINT64_C(5317839629434575638)},
        {UINT64_C(3576754321338143888), UINT64_C(11734850930654231305), UINT64_C(8073191253705007031), UINT64_C(5199019502630271206)},
        {UINT64_C(2193393763503187952), UINT64_C(3760040382739881176), UINT64_C(4395888683214695098), UINT64_C(1876127836792858977)},
        {UINT64_C(4402180547558184631), UINT64_C(14865680229259138542), UINT64_C(18402894886020912203), UINT64_C(3556038804589111293)},
        {UINT64_C(3333953653509690703), UINT64_C(7941459306008404834), UINT64_C(8513434601208093336), UINT64_C(3109961902298295669)},
        {UINT64_C(1651049162126695537), UINT64_C(13610822607695491168), UINT64_C(18025378285816230619), UINT64_C(1246694349819751138)},
        {UINT64_C(11914264628501797171), UINT64_C(1167887490757209588), UINT64_C(10324049099435107377), UINT64_C(1347777455064572611)},
        {UINT64_C(16344121126453470352), UINT64_C(4472919686863950750), UINT64_C(10222060743245200300), UINT64_C(7151781131736238962)},
        {UINT64_C(5387590375977475552), UINT64_C(788149212278504405), UINT64_C(18008872301659405168), UINT64_C(235785175205814281)},
        {UINT64_C(6724289423545220646), UINT64_C(13114492347121697151), UINT64_C(14890747259059654542), UINT64_C(5922177084246882648)},
        {UINT64_C(13373566316839044457), UINT64_C(8879634042234576536), UINT64_C(8535664209092769703), UINT64_C(13912493723286649495)},
        {UINT64_C(15567199961704485458), UINT64_C(790958119663664383), UINT64_C(5084750570288444125), UINT64_C(2421555008437624246)},
        {UINT64_C(7419001004436684943), UINT64_C(6708424519966836298), UINT64_C(4714413938819727057), UINT64_C(10556944913556258694)},
        {UINT64_C(5040442285948902575), UINT64_C(18084788378243005266), UINT64_C(10974318202876086053), UINT64_C(8306241024635485423)},
        {UINT64_C(9265426699833264073), UINT64_C(15689261916704666395), UINT64_C(10375013070689594198), UINT64_C(14011327530216836462)},
        {UINT64_C(2139907319578140909), UINT64_C(8136590057783446881), UINT64_C(7500733402670583555), UINT64_C(2321312821871294162)},
        {UINT64_C(6323136768283251790), UINT64_C(1295930199889866185), UINT64_C(16982424024148257253), UINT64_C(482519096472934706)},
        {UINT64_C(10690802931984592492), UINT64_C(7496641724724257398), UINT64_C(17403890029498266899), UINT64_C(4605011821775491314)},
        {UINT64_C(4469660348879681095), UINT64_C(8516631372207237694), UINT64_C(6874868164021286719), UINT64_C(5537044295568483525)},
        {UINT64_C(5573555655115263432), UINT64_C(18107274190249184565), UINT64_C(11011598300432387091), UINT64_C(9165054673109819465)},
        {UINT64_C(7222319169029362094), UINT64_C(15019825034102552249), UINT64_C(9663405630710797359), UINT64_C(11225645947689230535)},
        {UINT64_C(18086543665120355758), UINT64_C(13563860309568533904), UINT64_C(17422563291214423165), UINT64_C(14080784076147509526)},
        {UINT64_C(9457301505722790029), UINT64_C(8684637088605052464), UINT64_C(6673808338035007249), UINT64_C(12306801042911478778)},
        {UINT64_C(478196942572200737), UINT64_C(5926680701975044063), UINT64_C(13242705656550605479), UINT64_C(214013711758685137)},
        {UINT64_C(6356896612040073924), UINT64_C(10976006965984657529), UINT64_C(5501331282556343705), UINT64_C(12682992154468842168)},
        {UINT64_C(17731015474620576287), UINT64_C(16633189284393683856), UINT64_C(17609552465529137238), UINT64_C(16747917766291490535)},
        {UINT64_C(3323184603944988107), UINT64_C(1467691614061155741), UINT64_C(17421996682608099671), UINT64_C(279957014344755712)},
        {UINT64_C(8066978420283522533), UINT64_C(2669665177215874351), UINT64_C(8657854057359619029), UINT64_C(2487467590849031521)},
        {UINT64_C(7087317312551316161), UINT64_C(9296101970609295537), UINT64_C(15552217205880452545), UINT64_C(4236336437651481058)},
        {UINT64_C(149161439909437549), UINT64_C(12653361822772916335), UINT64_C(6312422626473325780), UINT64_C(298996721364072792)},
        {UINT64_C(2428095668076420780), UINT64_C(3612283104166096838), UINT64_C(1398984765939576625), UINT64_C(6269524279773445220)},
        {UINT64_C(3702931556482551646), UINT64_C(17935184223132505909), UINT64_C(10663653905084011093), UINT64_C(6227955278959528145)},
        {UINT64_C(7478949297565446917), UINT64_C(7866003638515789895), UINT64_C(3300122623298330814), UINT64_C(17826441348451293504)},
        {UINT64_C(14247826920245603850), UINT64_C(2697069777981329138), UINT64_C(11931701012313418831), UINT64_C(3220612329193169253)},
        {UINT64_C(2498380256149931881), UINT64_C(6761890451729251146), UINT64_C(1115597457911066692), UINT64_C(15143252146237719899)},
        {UINT64_C(7828193967564529283), UINT64_C(9880927881718847470), UINT64_C(11846002557546224343), UINT64_C(6529613653370158610)},
        {UINT64_C(6214066262560032662), UINT64_C(16485613433199094354), UINT64_C(9538146919947442058), UINT64_C(10740314142006679610)},
        {UINT64_C(14298149169146115044), UINT64_C(3737047299879441666), UINT64_C(8218621499783646327), UINT64_C(6501438197055014825)},
        {UINT64_C(1095516758617109798), UINT64_C(7490593120216781071), UINT64_C(5700481859637353261), UINT64_C(1439539761240737474)},
        {UINT64_C(15373444558689683391), UINT64_C(4231910816750166505), UINT64_C(14801073413113003513), UINT64_C(4395562707025615580)},
        {UINT64_C(10106625895315884819), UINT64_C(17221614411149248418), UINT64_C(15529904894041397511), UINT64_C(11207564718161773183)},
        {UINT64_C(16423117984575214778), UINT64_C(94390385022717459), UINT64_C(1324288493546249989), UINT64_C(1170579097675615969)},
        {UINT64_C(16791169405284609470), UINT64_C(7104127463991547670), UINT64_C(8394416429843823345), UINT64_C(14210232327827977783)},
        {UINT64_C(4385777912928194178), UINT64_C(562528067617759459), UINT64_C(11784420815340406162), UINT64_C(209354639741697103)},
        {UINT64_C(971822975596024527), UINT64_C(16481853844058265154), UINT64_C(2182078632189916490), UINT64_C(7340452360324201878)},
        {UINT64_C(17952351993831706128), UINT64_C(1420236397768257395), UINT64_C(15293689642071976487), UINT64_C(1667130975186513562)},
        {UINT64_C(1909685357616283548), UINT64_C(8934412800163807423), UINT64_C(17865187148563121628), UINT64_C(955037143551255441)},
        {UINT64_C(16247265974943597488), UINT64_C(3422806726459355766), UINT64_C(5916354701890744266), UINT64_C(9399580327365931196)},
        {UINT64_C(1629754734107076070), UINT64_C(4088819042837414060), UINT64_C(5488936949394403014), UINT64_C(1214036935276994963)},
    };
    for (size_t i = 0; i < sizeof(time_vectors) / sizeof(time_vectors[0]); ++i) {
        g.now = time_vectors[i][0];
        CHECK(llg_time_scaled(time_vectors[i][1], time_vectors[i][2]) == time_vectors[i][3]);
    }
    g.now = 0;
    double result = 0.0;
    CHECK(llg_plusarg_integral_real("0x1z", 4, 'h', &result) && result == 16.0);
    CHECK(llg_plusarg_integral_real("-0x1z", 5, 'h', &result) && result == 0.0);
    CHECK(llg_plusarg_integral_real("-18446744073709551615", 21, 'd', &result));
    CHECK(result == -(double)UINT64_MAX);
    CHECK(llg_plusarg_integral_real("123_x", 5, 'd', &result) && result == 0.0);
    CHECK(!llg_plusarg_integral_real("123q", 4, 'd', &result));
    CHECK(!llg_plusarg_integral_real("0x", 2, 'h', &result));
    size_t digits = 300000;
    char* long_binary = malloc(digits + 1u);
    CHECK(long_binary != NULL);
    memset(long_binary, '0', digits);
    long_binary[digits - 1u] = '1';
    long_binary[digits] = '\0';
    CHECK(llg_plusarg_integral_real(long_binary, digits, 'b', &result) && result == 1.0);
    free(long_binary);
    for (unsigned i = 1; i < 1000; ++i) {
        char text[64];
        uint64_t magnitude = (uint64_t)i * UINT64_C(14793720598743);
        int length = snprintf(text, sizeof(text), "%llu", (unsigned long long)magnitude);
        CHECK(llg_plusarg_integral_real(text, (size_t)length, 'd', &result));
        CHECK(result == (double)magnitude);
        length = snprintf(text, sizeof(text), "%llo", (unsigned long long)magnitude);
        CHECK(llg_plusarg_integral_real(text, (size_t)length, 'o', &result));
        CHECK(result == (double)magnitude);
    }
    sv4_t wide = sv4_zero(65537, 0);
    wide.bits[1024] = 1;
    for (unsigned i = 0; i < 50; ++i) {
        llg_fmt_arg_t arg = {.kind = LLG_FMT_PACKED, .value.packed = sv4_clone(&wide)};
        llg_string_t text = llg_string_format_typed(llg_string_bytes("%b", 2), &arg, 1, "top");
        CHECK(text.len == 65537 && text.data[0] == '1');
        CHECK(arg.value.packed.bits == NULL);
        llg_string_destroy(&text);
        CHECK(value_test_live() == 1);
    }
    sv4_destroy(&wide);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
}

int main(void) {
    check_file_messages();
    check_time_and_io();
    check_nba_and_scopes();
    check_frames();
    check_inertial_and_force();
    check_sequence_snapshots();
    check_sampling_mailboxes_and_reinit();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("scheduler owners, captures, cancellation and reinit: OK");
    return 0;
}
