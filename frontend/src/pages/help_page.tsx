import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";


/*
import { Item } from '../common/types';
import { loadData } from '../common/utils';
*/

interface Props {
    navigate: any
    t: any
}


export class HelpInnerPage extends react.Component<Props> {
    constructor(props: Props) {
        super(props);
        console.log("Constructing page");
    }

    render = () => {
        return (
        <>
                Nada
        </>
        );
    }
};




export default function HelpPage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <HelpInnerPage navigate={navigate} t={t} />;
}


