import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography } from 'antd';
const { Text } = Typography;

/*
import { Item } from '../common/types';
import { loadData } from '../common/utils';
*/

interface Props {
    navigate: any
    t: any
}


export class HomeInnerPage extends react.Component<Props> {
    constructor(props: Props) {
        super(props);
        console.log("Constructing page");
    }

    render = () => {
        return (
            <Flex justify="center" align="center" >
                <Text>Welcome to the Home Page</Text>
            </Flex>

        );
    }
};




export default function HomePage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <HomeInnerPage navigate={navigate} t={t} />;
}

